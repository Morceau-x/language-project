%grmtools{yacckind: Grmtools, recoverer: RecoveryKind::None}

// The Maxim grammar, translated production-for-production from the happy
// grammar in compiler/src/Maxim/Parser/Grammar.y. Policy: zero conflicts —
// every conflict is engineered away or resolved by an explicit precedence,
// never shrugged at (grmtools fails the build on any conflict, which IS
// happy's `%expect 0`).
//
// The two load-bearing constructions:
//
// * Cover grammar for `(…)`: parsed once as CoverParens, then validated
//   into a lambda parameter list (when followed by `=>`) or a single
//   parenthesized expression (the ECMAScript technique).
//
// * Juxtaposed match arms and block statements: no separators, no newline
//   tokens. Tokens that continue an expression and tokens that start a
//   pattern/statement are disjoint, except `-` (resolved greedily toward
//   the expression via the low-precedence P_LOW marker — negative
//   literal patterns are banned for exactly this reason) and `(`
//   (resolved by the lexer's LParenBOL split: a call's paren must be
//   on the same line as the callee).
//
// Happy's two entry points (pModule / pDeclSeg) become one start rule
// dispatched by a synthetic zero-width mode token the driver prepends.

%start Entry
%parse-param ctx: &ParseCtx<'_>

%token MODE_MODULE MODE_DECLSEG
%token lower upper hole int float string
%token stringBegin stringMid stringEnd
%token jsxTagStart jsxClose jsxTagEnd jsxSelfClose jsxAttrName jsxText dollarBrace
%token lparenBOL eof

%nonassoc P_LOW
%right '=>'
%left '|>'
%left '||'
%left '&&'
%nonassoc '==' '!='
%nonassoc '<' '>' '<=' '>='
%right '++'
%left '+' '-'
%left '*' '/' '~/' '%'
%right NEG
// '**' sits below NEG so exponentiation binds tighter than unary minus:
// at `- x . ** y` the shift wins and `-x ** y` means `-(x ** y)` (Python).
// The right operand can still be unary (`x ** -y`): after `**` only an
// expression can start, so there is no competing reduce.
%right '**'
// A same-line '(' after an expression is always a call, and a '?' after
// an expression always unwraps it: both shifts must beat the "finish
// this expression, start a new statement" reduce.
%nonassoc '(' '?'

%expect-unused 'P_LOW' 'NEG'

%%

Entry -> EntryOut:
    MODE_MODULE Module    { EntryOut::Module($2) }
  | MODE_DECLSEG DeclSeg  { EntryOut::Decl($2) }
  ;

Module -> CstModule:
    'module' upper Decls eof
      { CstModule { module_kw: ctx.t($1), name: ctx.t($2), decls: $3, eof: ctx.t($4) } }
  ;

Decls -> Vec<CstDecl>:
    { Vec::new() }
  | Decls Decl { let mut v = $1; v.push($2); v }
  ;

DeclSeg -> CstDecl:
    Decl eof { $1 }
  ;

Decl -> CstDecl:
    'import' upper
      { CstDecl::Import(ctx.t($1), ctx.t($2)) }
  | 'type' upper OptTyParams '=' TypeRhs
      { CstDecl::Type(ctx.t($1), ctx.t($2), $3, ctx.t($4), $5) }
  | 'fn' lower ParamParens OptRetAnn Block
      { CstDecl::Fn(ctx.t($1), ctx.t($2), $3, $4, $5) }
  | 'cast' ParamParens OptRetAnn Block
      { CstDecl::Cast(ctx.t($1), $2, $3, $4) }
  | 'component' upper ParamParens
      { CstDecl::Component(ctx.t($1), ctx.t($2), $3) }
  | 'let' lower OptTyAnn '=' Expr
      { CstDecl::Let(ctx.t($1), ctx.t($2), $3, ctx.t($4), $5) }
  ;

// Type declarations -----------------------------------------------------------

OptTyParams -> Option<CstTyParams>:
    { None }
  | '<' TyParams '>' { Some(CstTyParams(ctx.t($1), $2, ctx.t($3))) }
  ;

TyParams -> Separated<Token>:
    lower                { Separated::single(ctx.t($1)) }
  | TyParams ',' lower   { $1.snoc(ctx.t($2), ctx.t($3)) }
  ;

TypeRhs -> CstTypeRhs:
    OptBar Variants   { CstTypeRhs::Variants($1, $2) }
  | '{' Fields '}'    { CstTypeRhs::Record(Wrapped { open: ctx.t($1), value: $2, close: ctx.t($3) }) }
  ;

OptBar -> Option<Token>:
    { None }
  | '|' { Some(ctx.t($1)) }
  ;

Variants -> Separated<CstVariant>:
    Variant               { Separated::single($1) }
  | Variants '|' Variant  { $1.snoc(ctx.t($2), $3) }
  ;

Variant -> CstVariant:
    upper                       { CstVariant(ctx.t($1), None) }
  | upper AnyLParen Types ')'   { CstVariant(ctx.t($1), Some(Wrapped { open: $2, value: $3, close: ctx.t($4) })) }
  ;

Fields -> Separated<CstField>:
    Field             { Separated::single($1) }
  | Fields ',' Field  { $1.snoc(ctx.t($2), $3) }
  ;

Field -> CstField:
    lower ':' Type { CstField(ctx.t($1), ctx.t($2), $3) }
  ;

// Types -----------------------------------------------------------------------

Type -> CstType:
    upper OptTyArgs { CstType::Name(ctx.t($1), $2) }
  | lower           { CstType::Name(ctx.t($1), None) }
  | AnyLParen OptLabTypes ')' '->' Type
      { CstType::Fun(Box::new(Wrapped { open: $1, value: $2, close: ctx.t($3) }), ctx.t($4), Box::new($5)) }
  ;

// Function-type elements may carry a call label. `:` never follows a
// complete Type, so `lower . ':'` (shift, labeled) vs the Type -> lower
// reduction is decided by one token of lookahead — no conflict.
OptLabTypes -> Option<Separated<CstLabType>>:
    { None }
  | LabTypes { Some($1) }
  ;

LabTypes -> Separated<CstLabType>:
    LabType               { Separated::single($1) }
  | LabTypes ',' LabType  { $1.snoc(ctx.t($2), $3) }
  ;

LabType -> CstLabType:
    Type            { CstLabType(None, $1) }
  | lower ':' Type  { CstLabType(Some((ctx.t($1), ctx.t($2))), $3) }
  ;

OptTyArgs -> Option<Box<CstTyArgs>>:
    { None }
  | '<' Types '>' { Some(Box::new(CstTyArgs(ctx.t($1), $2, ctx.t($3)))) }
  ;

Types -> Separated<CstType>:
    Type            { Separated::single($1) }
  | Types ',' Type  { $1.snoc(ctx.t($2), $3) }
  ;

// Functions -------------------------------------------------------------------

ParamParens -> ParamParens:
    AnyLParen OptParams ')' { Wrapped { open: $1, value: $2, close: ctx.t($3) } }
  ;

AnyLParen -> Token:
    '('       { ctx.t($1) }
  | lparenBOL { ctx.t($1) }
  ;

OptParams -> Option<Separated<CstParam>>:
    { None }
  | Params { Some($1) }
  ;

Params -> Separated<CstParam>:
    Param             { Separated::single($1) }
  | Params ',' Param  { $1.snoc(ctx.t($2), $3) }
  ;

Param -> CstParam:
    lower OptTyAnn { CstParam(ctx.t($1), $2) }
  | hole OptTyAnn  { CstParam(ctx.t($1), $2) }
  ;

OptTyAnn -> Option<(Token, CstType)>:
    { None }
  | ':' Type { Some((ctx.t($1), $2)) }
  ;

OptRetAnn -> Option<(Token, CstType)>:
    { None }
  | '->' Type { Some((ctx.t($1), $2)) }
  ;

// Patterns --------------------------------------------------------------------

Pat -> CstPat:
    hole    { CstPat::Wild(ctx.t($1)) }
  | lower   { CstPat::Var(ctx.t($1)) }
  | int     { CstPat::Lit(ctx.t($1)) }
  | float   { CstPat::Lit(ctx.t($1)) }
  | string  { CstPat::Lit(ctx.t($1)) }
  | upper OptPatArgs            { CstPat::Ctor(None, ctx.t($1), $2) }
  | upper '.' upper OptPatArgs  { CstPat::Ctor(Some((ctx.t($1), ctx.t($2))), ctx.t($3), $4) }
  | '[' ']'                 { CstPat::List(Wrapped { open: ctx.t($1), value: None, close: ctx.t($2) }) }
  | '[' ListPatInner ']'    { CstPat::List(Wrapped { open: ctx.t($1), value: Some(Box::new($2)), close: ctx.t($3) }) }
  ;

OptPatArgs -> Option<Box<Wrapped<Separated<CstPat>>>>:
    { None }
  | AnyLParen Pats ')' { Some(Box::new(Wrapped { open: $1, value: $2, close: ctx.t($3) })) }
  ;

Pats -> Separated<CstPat>:
    Pat           { Separated::single($1) }
  | Pats ',' Pat  { $1.snoc(ctx.t($2), $3) }
  ;

ListPatInner -> CstListPat:
    Pats                        { CstListPat { elems: $1, rest: None } }
  | Pats ',' '..' RestBinder    { CstListPat { elems: $1, rest: Some((ctx.t($2), ctx.t($3), $4)) } }
  ;

RestBinder -> Token:
    lower { ctx.t($1) }
  | hole  { ctx.t($1) }
  ;

// Blocks and statements -------------------------------------------------------

Block -> CstBlock:
    '{' Stmts '}' { CstBlock(ctx.t($1), $2, ctx.t($3)) }
  ;

Stmts -> Vec<CstStmt>:
    { Vec::new() }
  | Stmts Stmt { let mut v = $1; v.push($2); v }
  ;

Stmt -> CstStmt:
    'let' lower OptTyAnn '=' Expr %prec P_LOW
      { CstStmt::Let(ctx.t($1), ctx.t($2), $3, ctx.t($4), $5) }
  | Expr %prec P_LOW { CstStmt::Expr($1) }
  ;

// Expressions -----------------------------------------------------------------

Expr -> CstExpr:
    // The arrow lambda is gone; the production stays so `(x) => e` parses
    // (keeping the grammar's conflict structure intact) and then reports
    // the fn-form rewrite.
    CoverParens '=>' Expr   { ctx.arrow_lambda_error(&ctx.t($2)) }
  | Expr '|>' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '||' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '&&' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '==' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '!=' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '<' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '>' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '<=' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '>=' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '++' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '+' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '-' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '*' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '/' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '~/' Expr          { cebin($1, ctx.t($2), $3) }
  | Expr '%' Expr           { cebin($1, ctx.t($2), $3) }
  | Expr '**' Expr          { cebin($1, ctx.t($2), $3) }
  | '-' Expr %prec NEG      { CstExpr::Unary(ctx.t($1), Box::new($2)) }
  | '!' Expr %prec NEG      { CstExpr::Unary(ctx.t($1), Box::new($2)) }
  | Postfix %prec P_LOW     { $1 }
  ;

Postfix -> CstExpr:
    Atom                    { $1 }
  | Postfix '.' lower       { CstExpr::Dot(Box::new($1), ctx.t($2), ctx.t($3)) }
  | Postfix '.' upper       { CstExpr::Dot(Box::new($1), ctx.t($2), ctx.t($3)) }
  | Postfix '(' OptArgs ')' { CstExpr::Call(Box::new($1), Wrapped { open: ctx.t($2), value: $3, close: ctx.t($4) }) }
  | Postfix '?'             { CstExpr::Try(Box::new($1), ctx.t($2)) }
    // v1 cast targets are a bare type NAME: a qualified (`M.T`) or generic
    // (`T<a>`) target would need two-token lookahead at `.`/`<` to keep
    // field access and comparison unambiguous.
  | Postfix '::' upper      { CstExpr::Cast(Box::new($1), ctx.t($2), ctx.t($3)) }
  ;

Atom -> CstExpr:
    lower   { CstExpr::Var(ctx.t($1)) }
  | upper   { CstExpr::Con(ctx.t($1)) }
  | int     { CstExpr::Lit(ctx.t($1)) }
  | float   { CstExpr::Lit(ctx.t($1)) }
  | string  { CstExpr::Lit(ctx.t($1)) }
  | hole    { CstExpr::Hole(ctx.t($1)) }
  | CoverParens %prec P_LOW { ctx.validate_group($1) }
  | '[' ']'               { CstExpr::List(Wrapped { open: ctx.t($1), value: None, close: ctx.t($2) }) }
  | '[' ListExprInner ']' { CstExpr::List(Wrapped { open: ctx.t($1), value: Some(Box::new($2)), close: ctx.t($3) }) }
  | Block { CstExpr::Block($1) }
  | 'fn' ParamParens OptRetAnn Block { CstExpr::FnLam(ctx.t($1), $2, $3, $4) }
  | If { $1 }
  | 'match' Expr '{' Arms '}'
      { CstExpr::Match(ctx.t($1), Box::new($2), ctx.t($3), $4, ctx.t($5)) }
  | stringBegin InterpTail { CstExpr::Interp(ctx.t($1), $2) }
  | Jsx { CstExpr::Jsx($1) }
  ;

// "chunk${ expr }chunk${ expr }chunk": each hole's expression followed by
// the mid/end chunk that closes it.
InterpTail -> Vec<(CstExpr, Token)>:
    Expr stringEnd            { vec![($1, ctx.t($2))] }
  | Expr stringMid InterpTail { let mut v = $3; v.insert(0, ($1, ctx.t($2))); v }
  ;

// JSX ------------------------------------------------------------------------
// Every alternative below begins with a terminal the lexer only produces
// inside an element (or the fresh `jsxTagStart`), so none of this can
// conflict with the expression grammar: the `<` decision was made at lex
// time. The one shared prefix — `'{'` starting a splice vs a spread —
// resolves one token later on `'..'`, which never starts an expression.

Jsx -> CstJsx:
    jsxTagStart JsxAttrs jsxSelfClose
      { CstJsx::SelfClose(ctx.t($1), $2, ctx.t($3)) }
  | jsxTagStart JsxAttrs jsxTagEnd JsxChildren jsxClose
      { CstJsx::Element(ctx.t($1), $2, ctx.t($3), $4, ctx.t($5)) }
  ;

JsxAttrs -> Vec<CstJsxAttr>:
    { Vec::new() }
  | JsxAttrs JsxAttr { let mut v = $1; v.push($2); v }
  ;

JsxAttr -> CstJsxAttr:
    jsxAttrName { CstJsxAttr::Bare(ctx.t($1)) }
  | jsxAttrName '=' string
      { CstJsxAttr::Value(ctx.t($1), ctx.t($2), CstJsxAttrValue::Str(ctx.t($3))) }
  | jsxAttrName '=' stringBegin InterpTail
      { CstJsxAttr::Value(ctx.t($1), ctx.t($2), CstJsxAttrValue::Interp(ctx.t($3), $4)) }
  | jsxAttrName '=' '{' Expr '}'
      { CstJsxAttr::Value(ctx.t($1), ctx.t($2), CstJsxAttrValue::Expr(Wrapped { open: ctx.t($3), value: Box::new($4), close: ctx.t($5) })) }
  ;

JsxChildren -> Vec<CstJsxChild>:
    { Vec::new() }
  | JsxChildren JsxChild { let mut v = $1; v.push($2); v }
  ;

JsxChild -> CstJsxChild:
    jsxText { CstJsxChild::Text(ctx.t($1)) }
  | Jsx     { CstJsxChild::Elem($1) }
  | '{' Expr '}'
      { CstJsxChild::Splice(Wrapped { open: ctx.t($1), value: Box::new($2), close: ctx.t($3) }) }
  | '{' '..' Expr '}'
      { CstJsxChild::Spread(ctx.t($1), ctx.t($2), Box::new($3), ctx.t($4)) }
  | dollarBrace Expr '}'
      { CstJsxChild::Hole(ctx.t($1), Box::new($2), ctx.t($3)) }
  ;

If -> CstExpr:
    'if' Expr Block 'else' ElseTail
      { CstExpr::If(ctx.t($1), Box::new($2), $3, ctx.t($4), Box::new($5)) }
  ;

ElseTail -> CstElse:
    Block { CstElse::Block($1) }
  | If    { CstElse::If(Box::new($1)) }
  ;

CoverParens -> Wrapped<Option<Separated<CstExpr>>>:
    AnyLParen ')'            { Wrapped { open: $1, value: None, close: ctx.t($2) } }
  | AnyLParen CoverElems ')' { Wrapped { open: $1, value: Some($2), close: ctx.t($3) } }
  ;

CoverElems -> Separated<CstExpr>:
    Expr                { Separated::single($1) }
  | CoverElems ',' Expr { $1.snoc(ctx.t($2), $3) }
  ;

ListExprInner -> CstListExpr:
    Exprs               { CstListExpr { elems: $1, spread: None } }
  | Exprs ',' '..' Expr { CstListExpr { elems: $1, spread: Some((ctx.t($2), ctx.t($3), $4)) } }
  ;

Exprs -> Separated<CstExpr>:
    Expr            { Separated::single($1) }
  | Exprs ',' Expr  { $1.snoc(ctx.t($2), $3) }
  ;

OptArgs -> Option<Separated<CstArg>>:
    { None }
  | Args { Some($1) }
  ;

Args -> Separated<CstArg>:
    Arg           { Separated::single($1) }
  | Args ',' Arg  { $1.snoc(ctx.t($2), $3) }
  ;

Arg -> CstArg:
    Expr            { CstArg::Pos(Box::new($1)) }
  | lower ':' Expr  { CstArg::Lab(ctx.t($1), ctx.t($2), Box::new($3)) }
  | '..' Expr       { CstArg::Spread(ctx.t($1), Box::new($2)) }
  ;

// Match arms ------------------------------------------------------------------

Arms -> Vec<CstArm>:
    Arm       { vec![$1] }
  | Arms Arm  { let mut v = $1; v.push($2); v }
  ;

Arm -> CstArm:
    Pat '->' Expr %prec P_LOW
      { CstArm($1, None, ctx.t($2), $3) }
  | Pat 'if' Expr '->' Expr %prec P_LOW
      { CstArm($1, Some((ctx.t($2), $3)), ctx.t($4), $5) }
  ;

%%

use crate::cst::{
    CstArg, CstArm, CstBlock, CstDecl, CstElse, CstExpr, CstField, CstJsx, CstJsxAttr,
    CstJsxAttrValue, CstJsxChild, CstLabType, CstListExpr, CstListPat, CstModule, CstParam,
    CstPat, CstStmt, CstTyArgs, CstTyParams, CstType, CstTypeRhs, CstVariant, ParamParens,
    Separated, Wrapped,
};
use crate::lexer::token::Token;
use crate::parser::grammar::{EntryOut, ParseCtx};

fn cebin(l: CstExpr, op: Token, r: CstExpr) -> CstExpr {
    CstExpr::Bin(Box::new(l), op, Box::new(r))
}
