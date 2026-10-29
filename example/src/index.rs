//! The occurrence index over the renamed AST: every place a `Name` is
//! referenced, with the span the editor sees. `Name::def_span` on the
//! indexed name is the go-to-definition target.

use crate::ast::*;
use crate::names::Name;
use crate::span::Span;

pub fn name_occurrences(m: &Module<Name>) -> Vec<(Span, Name)> {
    let mut out = Vec::new();
    for d in &m.decls {
        match d {
            Decl::Type(_) => {}
            Decl::Fn(fnd) => expr(&fnd.body, &mut out),
            Decl::Let(_, _, _, e) => expr(e, &mut out),
            Decl::Cast(cd) => expr(&cd.body, &mut out),
            // The manifest's wired names are ordinary value references.
            Decl::Component(cd) => {
                for w in [&cd.init, &cd.update].into_iter().flatten() {
                    out.push(w.clone());
                }
                out.push(cd.view.clone());
            }
        }
    }
    out
}

fn expr(e: &Expr<Name>, out: &mut Vec<(Span, Name)>) {
    match e {
        // Synthesized by Core lowering — never present here (totality arm).
        Expr::Instance(_, _, mp, outs) => {
            if let Some(p) = mp {
                expr(p, out);
            }
            for (_, e2) in outs {
                expr(e2, out);
            }
        }
        Expr::Var(sp, n) => out.push((*sp, n.clone())),
        Expr::Con(sp, n) => out.push((*sp, n.clone())),
        Expr::Lit(_, _) => {}
        Expr::Dot(_, e2, _, _) => expr(e2, out),
        Expr::Field(_, e2, _, _) => expr(e2, out),
        Expr::Call(_, f, args) => {
            expr(f, out);
            for a in args {
                expr(a.expr(), out);
            }
        }
        Expr::Update(_, n, ctor_sp, base, fields) => {
            out.push((*ctor_sp, n.clone()));
            expr(base, out);
            for (_, _, e2) in fields {
                expr(e2, out);
            }
        }
        Expr::Bin(_, _, _, l, r) => {
            expr(l, out);
            expr(r, out);
        }
        Expr::Unary(_, _, e2) => expr(e2, out),
        Expr::Pipe(_, _, l, r) => {
            expr(l, out);
            expr(r, out);
        }
        Expr::If(_, c, t, e2) => {
            expr(c, out);
            expr(t, out);
            expr(e2, out);
        }
        Expr::Match(_, scrut, arms) => {
            expr(scrut, out);
            for a in arms {
                pat(&a.pat, out);
                if let Some(g) = &a.guard {
                    expr(g, out);
                }
                expr(&a.body, out);
            }
        }
        Expr::Lam(_, _, _, body) => expr(body, out),
        Expr::List(_, es, spread) => {
            for e2 in es {
                expr(e2, out);
            }
            if let Some(s) = spread {
                expr(s, out);
            }
        }
        Expr::Block(_, stmts, result) => {
            for s in stmts {
                match s {
                    Stmt::Let(_, _, _, e2) => expr(e2, out),
                    Stmt::Discard(e2) => expr(e2, out),
                }
            }
            expr(result, out);
        }
        Expr::Try(_, _, e2) => expr(e2, out),
        Expr::Cast(_, e2, _, _) => expr(e2, out),
        Expr::Interp(_, parts) => {
            for p in parts {
                if let InterpPart::Expr(e2) = p {
                    expr(e2, out);
                }
            }
        }
        Expr::Jsx(_, el) => jsx_elem(el, out),
    }
}

/// A component tag occurrence points at `Module.view`: go-to-definition
/// on `<Card` jumps to the view function.
fn jsx_elem(el: &JsxElem<Name>, out: &mut Vec<(Span, Name)>) {
    match &el.tag {
        JsxTag::Intrinsic(_, _) => {}
        JsxTag::Component(sp, n) => out.push((*sp, n.clone())),
    }
    for a in &el.attrs {
        match a {
            JsxAttr::Attr(_, _, v) => {
                if let Some(e2) = v {
                    expr(e2, out);
                }
            }
            JsxAttr::On(_, _, e2) => expr(e2, out),
        }
    }
    for c in &el.children {
        match c {
            JsxChild::Text(_, _) => {}
            JsxChild::Elem(el2) => jsx_elem(el2, out),
            JsxChild::Splice(_, e2) | JsxChild::Spread(_, e2) | JsxChild::Hole(_, e2) => {
                expr(e2, out)
            }
        }
    }
}

fn pat(p: &Pat<Name>, out: &mut Vec<(Span, Name)>) {
    match p {
        Pat::Ctor(sp, n, args) => {
            out.push((*sp, n.clone()));
            for a in args {
                pat(a, out);
            }
        }
        Pat::List(_, ps, _) => {
            for a in ps {
                pat(a, out);
            }
        }
        Pat::Wild(_) | Pat::Var(_, _) | Pat::Lit(_, _) => {}
    }
}
