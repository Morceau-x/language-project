//! The exact printer: reproduce the source from the CST, byte for byte.
//! `print_module . parse_module == id` is golden-tested and is the
//! foundation the formatter stands on.
//!
//! The `HasTokens` traversal also powers span computation for CST nodes.

use crate::cst::*;
use crate::lexer::token::Token;
use crate::maxim_panic;
use crate::span::Span;

/// Every CST node can visit its tokens in source order.
pub trait HasTokens {
    fn each_token(&self, f: &mut dyn FnMut(&Token));
}

pub fn print_module(m: &CstModule) -> String {
    let mut out = String::new();
    m.each_token(&mut |t| t.write_full_text(&mut out));
    out
}

pub fn print_tokens(toks: &[Token]) -> String {
    let mut out = String::new();
    for t in toks {
        t.write_full_text(&mut out);
    }
    out
}

/// The span of a CST node: hull of its first and last token. Total for
/// every real node (all CST nodes contain at least one token).
pub fn span_of(x: &dyn HasTokens) -> Span {
    let mut first: Option<Span> = None;
    let mut last: Option<Span> = None;
    x.each_token(&mut |t| {
        if first.is_none() {
            first = Some(t.span);
        }
        last = Some(t.span);
    });
    match (first, last) {
        (Some(a), Some(b)) => a.merge(b),
        _ => maxim_panic!("cst::print::span_of: node without tokens"),
    }
}

impl HasTokens for Token {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        f(self);
    }
}

impl<T: HasTokens> HasTokens for Vec<T> {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        for x in self {
            x.each_token(f);
        }
    }
}

impl<T: HasTokens> HasTokens for Option<T> {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        if let Some(x) = self {
            x.each_token(f);
        }
    }
}

impl<T: HasTokens + ?Sized> HasTokens for Box<T> {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        (**self).each_token(f);
    }
}

impl<T: HasTokens + ?Sized> HasTokens for &T {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        (**self).each_token(f);
    }
}

impl<A: HasTokens, B: HasTokens> HasTokens for (A, B) {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
    }
}

impl<A: HasTokens, B: HasTokens, C: HasTokens> HasTokens for (A, B, C) {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
    }
}

impl<T: HasTokens> HasTokens for Wrapped<T> {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.open.each_token(f);
        self.value.each_token(f);
        self.close.each_token(f);
    }
}

impl<T: HasTokens> HasTokens for Separated<T> {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.head.each_token(f);
        for (tok, x) in &self.tail {
            tok.each_token(f);
            x.each_token(f);
        }
    }
}

impl HasTokens for CstModule {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.module_kw.each_token(f);
        self.name.each_token(f);
        self.decls.each_token(f);
        self.eof.each_token(f);
    }
}

impl HasTokens for CstDecl {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstDecl::Import(kw, name) => {
                kw.each_token(f);
                name.each_token(f);
            }
            CstDecl::Type(kw, name, ps, eq, rhs) => {
                kw.each_token(f);
                name.each_token(f);
                ps.each_token(f);
                eq.each_token(f);
                rhs.each_token(f);
            }
            CstDecl::Fn(kw, name, params, ret, body) => {
                kw.each_token(f);
                name.each_token(f);
                params.each_token(f);
                ret.each_token(f);
                body.each_token(f);
            }
            CstDecl::Let(kw, name, ann, eq, e) => {
                kw.each_token(f);
                name.each_token(f);
                ann.each_token(f);
                eq.each_token(f);
                e.each_token(f);
            }
            CstDecl::Cast(kw, params, ret, body) => {
                kw.each_token(f);
                params.each_token(f);
                ret.each_token(f);
                body.each_token(f);
            }
            CstDecl::Component(kw, name, fields) => {
                kw.each_token(f);
                name.each_token(f);
                fields.each_token(f);
            }
            CstDecl::Error(ts) => {
                for t in ts {
                    f(t);
                }
            }
        }
    }
}

impl HasTokens for CstTyParams {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
    }
}

impl HasTokens for CstParam {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
    }
}

impl HasTokens for CstTypeRhs {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstTypeRhs::Variants(bar, vs) => {
                bar.each_token(f);
                vs.each_token(f);
            }
            CstTypeRhs::Record(w) => w.each_token(f),
        }
    }
}

impl HasTokens for CstVariant {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
    }
}

impl HasTokens for CstField {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
    }
}

impl HasTokens for CstType {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstType::Name(name, args) => {
                name.each_token(f);
                args.each_token(f);
            }
            CstType::Fun(params, arrow, ret) => {
                params.each_token(f);
                arrow.each_token(f);
                ret.each_token(f);
            }
        }
    }
}

impl HasTokens for CstLabType {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
    }
}

impl HasTokens for CstTyArgs {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
    }
}

impl HasTokens for CstPat {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstPat::Wild(t) | CstPat::Var(t) | CstPat::Lit(t) => t.each_token(f),
            CstPat::Ctor(q, name, args) => {
                q.each_token(f);
                name.each_token(f);
                args.each_token(f);
            }
            CstPat::List(w) => w.each_token(f),
        }
    }
}

impl HasTokens for CstListPat {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.elems.each_token(f);
        self.rest.each_token(f);
    }
}

impl HasTokens for CstExpr {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstExpr::Var(t) | CstExpr::Con(t) | CstExpr::Lit(t) | CstExpr::Hole(t) => {
                t.each_token(f)
            }
            CstExpr::Dot(e, dot, name) => {
                e.each_token(f);
                dot.each_token(f);
                name.each_token(f);
            }
            CstExpr::Call(e, args) => {
                e.each_token(f);
                args.each_token(f);
            }
            CstExpr::Bin(l, op, r) => {
                l.each_token(f);
                op.each_token(f);
                r.each_token(f);
            }
            CstExpr::Unary(op, e) => {
                op.each_token(f);
                e.each_token(f);
            }
            CstExpr::Group(w) => w.each_token(f),
            CstExpr::FnLam(kw, params, ret, body) => {
                kw.each_token(f);
                params.each_token(f);
                ret.each_token(f);
                body.each_token(f);
            }
            CstExpr::If(kw, cond, then_b, else_kw, else_t) => {
                kw.each_token(f);
                cond.each_token(f);
                then_b.each_token(f);
                else_kw.each_token(f);
                else_t.each_token(f);
            }
            CstExpr::Match(kw, scrut, o, arms, c) => {
                kw.each_token(f);
                scrut.each_token(f);
                o.each_token(f);
                arms.each_token(f);
                c.each_token(f);
            }
            CstExpr::List(w) => w.each_token(f),
            CstExpr::Block(b) => b.each_token(f),
            CstExpr::Try(e, q) => {
                e.each_token(f);
                q.each_token(f);
            }
            CstExpr::Cast(e, cc, ty) => {
                e.each_token(f);
                cc.each_token(f);
                ty.each_token(f);
            }
            CstExpr::Interp(begin, parts) => {
                begin.each_token(f);
                parts.each_token(f);
            }
            CstExpr::Jsx(j) => j.each_token(f),
        }
    }
}

impl HasTokens for CstJsx {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstJsx::SelfClose(open, attrs, close) => {
                open.each_token(f);
                attrs.each_token(f);
                close.each_token(f);
            }
            CstJsx::Element(open, attrs, gt, children, close) => {
                open.each_token(f);
                attrs.each_token(f);
                gt.each_token(f);
                children.each_token(f);
                close.each_token(f);
            }
        }
    }
}

impl HasTokens for CstJsxAttr {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstJsxAttr::Bare(name) => name.each_token(f),
            CstJsxAttr::Value(name, eq, value) => {
                name.each_token(f);
                eq.each_token(f);
                value.each_token(f);
            }
        }
    }
}

impl HasTokens for CstJsxAttrValue {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstJsxAttrValue::Str(t) => t.each_token(f),
            CstJsxAttrValue::Interp(begin, parts) => {
                begin.each_token(f);
                parts.each_token(f);
            }
            CstJsxAttrValue::Expr(w) => w.each_token(f),
        }
    }
}

impl HasTokens for CstJsxChild {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstJsxChild::Text(t) => t.each_token(f),
            CstJsxChild::Elem(j) => j.each_token(f),
            CstJsxChild::Splice(w) => w.each_token(f),
            CstJsxChild::Spread(o, dd, e, c) => {
                o.each_token(f);
                dd.each_token(f);
                e.each_token(f);
                c.each_token(f);
            }
            CstJsxChild::Hole(o, e, c) => {
                o.each_token(f);
                e.each_token(f);
                c.each_token(f);
            }
        }
    }
}

impl HasTokens for CstListExpr {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.elems.each_token(f);
        self.spread.each_token(f);
    }
}

impl HasTokens for CstArg {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstArg::Pos(e) => e.each_token(f),
            CstArg::Lab(name, colon, e) => {
                name.each_token(f);
                colon.each_token(f);
                e.each_token(f);
            }
            CstArg::Spread(dd, e) => {
                dd.each_token(f);
                e.each_token(f);
            }
        }
    }
}

impl HasTokens for CstArm {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
        self.3.each_token(f);
    }
}

impl HasTokens for CstBlock {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        self.0.each_token(f);
        self.1.each_token(f);
        self.2.each_token(f);
    }
}

impl HasTokens for CstStmt {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstStmt::Let(kw, name, ann, eq, e) => {
                kw.each_token(f);
                name.each_token(f);
                ann.each_token(f);
                eq.each_token(f);
                e.each_token(f);
            }
            CstStmt::Expr(e) => e.each_token(f),
        }
    }
}

impl HasTokens for CstElse {
    fn each_token(&self, f: &mut dyn FnMut(&Token)) {
        match self {
            CstElse::Block(b) => b.each_token(f),
            CstElse::If(e) => e.each_token(f),
        }
    }
}
