//! Lower the lossless CST to the surface AST: drop trivia, keep spans,
//! decode literal payloads, and validate the two things the grammar is too
//! permissive about (`_` outside lambda params, blocks without a result).

use crate::ast as a;
use crate::cst::print::span_of;
use crate::cst::*;
use crate::diag::Diagnostic;
use crate::lexer::token::{Operator, Token, TokenKind};
use crate::maxim_panic;
use crate::names::{ModuleName, RawName};
use crate::span::Span;

type Lower<T> = Result<T, Diagnostic>;

pub fn lower_module(m: &CstModule) -> Lower<a::Module<RawName>> {
    // Broken segments (error recovery) drop out here: their diagnostic is
    // already reported, and everything else proceeds without them.
    let healthy: Vec<&CstDecl> = m
        .decls
        .iter()
        .filter(|d| !matches!(d, CstDecl::Error(_)))
        .collect();
    let mut imports = Vec::new();
    let mut others = Vec::new();
    for d in healthy {
        match d {
            CstDecl::Import(_kw, name) => imports.push(a::Import {
                module: ModuleName(token_text(name)),
                span: name.span,
            }),
            d => others.push(d),
        }
    }
    let mut decls = Vec::with_capacity(others.len());
    for d in others {
        decls.push(lower_decl(d)?);
    }
    Ok(a::Module {
        name: ModuleName(token_text(&m.name)),
        name_span: m.name.span,
        imports,
        decls,
    })
}

fn lower_decl(d: &CstDecl) -> Lower<a::Decl<RawName>> {
    match d {
        CstDecl::Import(_, _) => maxim_panic!("lower_decl: imports are split out beforehand"),
        CstDecl::Error(_) => maxim_panic!("lower_decl: error segments are filtered out beforehand"),
        CstDecl::Type(kw, name, typarams, _eq, rhs) => {
            let rhs2 = lower_type_rhs(&token_text(name), rhs)?;
            Ok(a::Decl::Type(a::TypeDecl {
                span: kw.span.merge(span_of(rhs)),
                name: raw(name),
                params: typarams.as_ref().map_or_else(Vec::new, |tp| {
                    tp.1.iter().map(|t| (t.span, token_text(t))).collect()
                }),
                rhs: rhs2,
            }))
        }
        CstDecl::Fn(kw, name, params, ret, body) => {
            let ps: Vec<a::FnParam<RawName>> = params
                .value
                .iter()
                .flat_map(|sep| sep.iter())
                .map(lower_param)
                .collect();
            let body2 = lower_block(body)?;
            Ok(a::Decl::Fn(a::FnDef {
                span: kw.span.merge(span_of(body)),
                name: raw(name),
                params: ps,
                ret: ret.as_ref().map(lower_ann),
                body: body2,
            }))
        }
        CstDecl::Let(kw, name, ann, _eq, e) => {
            let e2 = lower_expr(e)?;
            Ok(a::Decl::Let(
                kw.span.merge(span_of(e)),
                raw(name),
                ann.as_ref().map(lower_ann),
                e2,
            ))
        }
        CstDecl::Cast(kw, params, ret, body) => {
            let bad = |sp: Span, msg: String| -> Diagnostic {
                Diagnostic::error("E1112", "INVALID CAST", sp, msg).with_hints(vec![
                    "a cast is `cast (value: Source) -> Target { … }`".to_string(),
                ])
            };
            let param_list: Vec<&CstParam> =
                params.value.iter().flat_map(|sep| sep.iter()).collect();
            let param = match &param_list[..] {
                [p] => lower_param(p),
                ps => {
                    return Err(bad(
                        span_of(params),
                        format!(
                            "a cast converts exactly one value, but this one takes {} parameters",
                            ps.len()
                        ),
                    ));
                }
            };
            let src_head = match &param.ann {
                Some(a::TypeAnn::Name(_, txt, _)) => txt.clone(),
                Some(ann) => {
                    return Err(bad(
                        ann.span(),
                        "a cast's source must be a named type".to_string(),
                    ));
                }
                None => {
                    return Err(bad(
                        param.span,
                        "the parameter needs a type annotation — it is the cast's source type"
                            .to_string(),
                    ));
                }
            };
            let (ret_ann, tgt_head) = match ret {
                Some(r) => match lower_ann(r) {
                    ann @ a::TypeAnn::Name(_, _, _) => {
                        let txt = match &ann {
                            a::TypeAnn::Name(_, txt, _) => txt.clone(),
                            _ => unreachable!(),
                        };
                        (ann, txt)
                    }
                    ann => {
                        return Err(bad(
                            ann.span(),
                            "a cast's target must be a named type".to_string(),
                        ));
                    }
                },
                None => {
                    return Err(bad(
                        span_of(d),
                        "a cast needs a return type — it is the cast's target".to_string(),
                    ));
                }
            };
            let body2 = lower_block(body)?;
            Ok(a::Decl::Cast(a::CastDef {
                span: kw.span.merge(span_of(body)),
                name: RawName::plain(format!("{}To{}", lower_first(&src_head), tgt_head)),
                src_head,
                tgt_head,
                param,
                ret: ret_ann,
                body: body2,
            }))
        }
        CstDecl::Component(kw, name, fields) => {
            let bad = |sp: Span, msg: String| -> Diagnostic {
                Diagnostic::error("E1118", "INVALID COMPONENT", sp, msg).with_hints(vec![
                    "A component is `component Name(init: init, update: update, view: view)` — `view` is mandatory, the rest optional."
                        .to_string(),
                ])
            };
            let decl_span = kw.span.merge(span_of(fields));
            let mut wires: Vec<(String, Span, (Span, RawName))> = Vec::new();
            for p in fields.value.iter().flat_map(|sep| sep.iter()) {
                let CstParam(lbl, ann) = p;
                match ann {
                    Some(annp) => match lower_ann(annp) {
                        a::TypeAnn::Var(vsp, txt) => {
                            wires.push((token_text(lbl), lbl.span, (vsp, RawName::plain(txt))));
                        }
                        other => {
                            return Err(bad(
                                other.span(),
                                format!("`{}:` must name a top-level function", token_text(lbl)),
                            ));
                        }
                    },
                    None => {
                        return Err(bad(
                            span_of(p),
                            format!(
                                "`{0}` needs a function to wire: `{0}: someFn`",
                                token_text(lbl)
                            ),
                        ));
                    }
                }
            }
            if let Some((l, sp)) = wires
                .iter()
                .find(|(l, _, _)| !matches!(l.as_str(), "init" | "update" | "view"))
                .map(|(l, sp, _)| (l.clone(), *sp))
            {
                return Err(bad(sp, format!("components have no `{l}:` slot")));
            }
            let pick = |l: &str| -> Lower<Option<(Span, RawName)>> {
                let hits: Vec<&(String, Span, (Span, RawName))> =
                    wires.iter().filter(|(l2, _, _)| l2 == l).collect();
                match &hits[..] {
                    [] => Ok(None),
                    [(_, _, w)] => Ok(Some(w.clone())),
                    [_, (_, sp2, _), ..] => Err(bad(*sp2, format!("`{l}:` is wired twice"))),
                }
            };
            let init_w = pick("init")?;
            let update_w = pick("update")?;
            let view_w = match pick("view")? {
                Some(w) => w,
                None => return Err(bad(decl_span, "a component needs a `view:`".to_string())),
            };
            Ok(a::Decl::Component(a::ComponentDef {
                span: decl_span,
                name: raw(name),
                init: init_w,
                update: update_w,
                view: view_w,
            }))
        }
    }
}

fn lower_first(t: &str) -> String {
    let mut cs = t.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

fn lower_type_rhs(type_name: &str, rhs: &CstTypeRhs) -> Lower<a::TypeRhs<RawName>> {
    match rhs {
        CstTypeRhs::Variants(_bar, vs) => Ok(a::TypeRhs::Variants(
            vs.iter()
                .map(|v| {
                    let CstVariant(name, args) = v;
                    a::Variant {
                        span: span_of(v),
                        name: raw(name),
                        args: args
                            .as_ref()
                            .map_or_else(Vec::new, |w| w.value.iter().map(lower_type).collect()),
                    }
                })
                .collect(),
        )),
        CstTypeRhs::Record(w) => Ok(a::TypeRhs::Record(
            RawName::plain(type_name),
            w.value
                .iter()
                .map(|CstField(name, _colon, ty)| a::Field {
                    span: name.span,
                    name: token_text(name),
                    ty: lower_type(ty),
                })
                .collect(),
        )),
    }
}

fn lower_param(p: &CstParam) -> a::FnParam<RawName> {
    let CstParam(name, ann) = p;
    a::FnParam {
        span: name.span,
        name: binder_of(name),
        ann: ann.as_ref().map(lower_ann),
    }
}

fn lower_ann((_tok, ty): &(Token, CstType)) -> a::TypeAnn {
    lower_type(ty)
}

fn lower_type(t: &CstType) -> a::TypeAnn {
    match t {
        CstType::Name(name, args) => match &name.kind {
            TokenKind::Upper(txt) => a::TypeAnn::Name(
                span_of(t),
                txt.clone(),
                args.as_ref()
                    .map_or_else(Vec::new, |ta| ta.1.iter().map(lower_type).collect()),
            ),
            _ => a::TypeAnn::Var(name.span, token_text(name)),
        },
        CstType::Fun(params, _arrow, ret) => a::TypeAnn::Fun(
            span_of(t),
            params.value.as_ref().map_or_else(Vec::new, |sep| {
                sep.iter()
                    .map(|CstLabType(mlbl, ty)| {
                        (mlbl.as_ref().map(|(l, _)| token_text(l)), lower_type(ty))
                    })
                    .collect()
            }),
            Box::new(lower_type(ret)),
        ),
    }
}

fn lower_block(b: &CstBlock) -> Lower<a::Expr<RawName>> {
    let CstBlock(_o, stmts, _c) = b;
    let mut ss = Vec::with_capacity(stmts.len());
    for s in stmts {
        ss.push(lower_stmt(s)?);
    }
    match ss.pop() {
        Some(a::Stmt::Discard(result)) => Ok(a::Expr::Block(span_of(b), ss, Box::new(result))),
        _ => Err(Diagnostic::error(
            "E1106",
            "MISSING RESULT",
            span_of(b),
            "this block doesn't end with a value",
        )
        .with_hints(vec![
            "Every block is an expression in Maxim; finish it with the value it produces."
                .to_string(),
        ])),
    }
}

fn lower_stmt(s: &CstStmt) -> Lower<a::Stmt<RawName>> {
    match s {
        CstStmt::Let(kw, name, ann, _eq, e) => {
            let e2 = lower_expr(e)?;
            Ok(a::Stmt::Let(
                kw.span.merge(span_of(e)),
                raw(name),
                ann.as_ref().map(lower_ann),
                e2,
            ))
        }
        CstStmt::Expr(e) => Ok(a::Stmt::Discard(lower_expr(e)?)),
    }
}

pub fn lower_expr(cst: &CstExpr) -> Lower<a::Expr<RawName>> {
    match cst {
        CstExpr::Var(t) => Ok(a::Expr::Var(t.span, raw(t))),
        CstExpr::Con(t) => Ok(a::Expr::Con(t.span, raw(t))),
        CstExpr::Lit(t) => Ok(a::Expr::Lit(t.span, lit_of(t))),
        CstExpr::Hole(t) => Err(Diagnostic::error(
            "E1103",
            "UNEXPECTED UNDERSCORE",
            t.span,
            "I found `_` where an expression should be",
        )
        .with_hints(vec![
            "`_` only makes sense as a lambda parameter, like `fn(_) { 0 }`.".to_string(),
        ])),
        CstExpr::Dot(e, _dot, name) => {
            let e2 = lower_expr(e)?;
            Ok(a::Expr::Dot(
                span_of(cst),
                Box::new(e2),
                name.span,
                token_text(name),
            ))
        }
        CstExpr::Call(e, args) => {
            let arg_list: Vec<&CstArg> = args.value.iter().flat_map(|sep| sep.iter()).collect();
            match &arg_list[..] {
                // A leading `..base` makes this a record update, not a call.
                [CstArg::Spread(_dd, base), rest @ ..] => {
                    let (ctor, ctor_sp) = match &**e {
                        CstExpr::Con(t) => (raw(t), t.span),
                        CstExpr::Dot(q, _dot, member)
                            if matches!(&**q, CstExpr::Con(_))
                                && matches!(member.kind, TokenKind::Upper(_)) =>
                        {
                            let CstExpr::Con(q) = &**q else {
                                unreachable!()
                            };
                            (
                                RawName {
                                    qualifier: Some(token_text(q)),
                                    base: token_text(member),
                                },
                                q.span.merge(member.span),
                            )
                        }
                        _ => {
                            return Err(Diagnostic::error(
                                "E1108",
                                "UPDATE NEEDS A CONSTRUCTOR",
                                span_of(&**e),
                                "`..` updates a record, and this is not a record constructor",
                            )
                            .with_hints(vec![
                                "Record update spreads into a constructor: `User(..u, age: 31)`."
                                    .to_string(),
                            ]));
                        }
                    };
                    let base2 = lower_expr(base)?;
                    let mut fields = Vec::with_capacity(rest.len());
                    for arg in rest {
                        fields.push(lower_update_field(arg)?);
                    }
                    Ok(a::Expr::Update(
                        span_of(cst),
                        ctor,
                        ctor_sp,
                        Box::new(base2),
                        fields,
                    ))
                }
                _ => {
                    let e2 = lower_expr(e)?;
                    let mut args2 = Vec::with_capacity(arg_list.len());
                    for arg in &arg_list {
                        args2.push(lower_arg(arg)?);
                    }
                    Ok(a::Expr::Call(span_of(cst), Box::new(e2), args2))
                }
            }
        }
        CstExpr::Bin(l, op, r) => {
            match &op.kind {
                // `xs |> stage?` unwraps the piped stage: the `?` hoists
                // over the pipe, so it lowers as (xs |> stage)?.
                // Parenthesizing the try — `xs |> (h?)` — blocks the hoist
                // and pipes into the unwrapped value instead. The same
                // parens opt a call out of subject PREPENDING: `xs |> f(a)`
                // is f(xs, a), while `xs |> (f(a))` applies the (say,
                // partial) value of f(a) to xs.
                TokenKind::Pipeline => {
                    let l2 = lower_expr(l)?;
                    let (stage, qs) = peel_try(r);
                    let style = match stage {
                        CstExpr::Call(_, _) => a::PipeStyle::Into,
                        _ => a::PipeStyle::Apply,
                    };
                    let stage2 = lower_expr(stage)?;
                    let piped = a::Expr::Pipe(
                        span_of(&**l).merge(span_of(stage)),
                        style,
                        Box::new(l2),
                        Box::new(stage2),
                    );
                    Ok(qs.into_iter().fold(piped, |acc, q| {
                        a::Expr::Try(acc.span().merge(q.span), q.span, Box::new(acc))
                    }))
                }
                // `++` still lexes and parses (losslessness, and the
                // formatter keeps working on legacy sources) but is no
                // longer part of the language.
                TokenKind::Op(Operator::Concat) => Err(Diagnostic::error(
                    "E1110",
                    "REMOVED OPERATOR",
                    op.span,
                    "`++` is no longer an operator",
                )
                .with_hints(vec![
                    "Build strings with interpolation instead: `\"${a}${b}\"`.".to_string(),
                ])),
                k => {
                    let l2 = lower_expr(l)?;
                    let r2 = lower_expr(r)?;
                    let sp = span_of(cst);
                    let bop = match k {
                        TokenKind::Lt => a::BinOp::Lt,
                        TokenKind::Gt => a::BinOp::Gt,
                        TokenKind::Op(o) => bin_of(*o),
                        k => maxim_panic!("lower_expr: impossible binary operator token {k:?}"),
                    };
                    Ok(a::Expr::Bin(sp, bop, op.span, Box::new(l2), Box::new(r2)))
                }
            }
        }
        CstExpr::Unary(op, e) => {
            let e2 = lower_expr(e)?;
            let un = match op.kind {
                TokenKind::Bang => a::UnOp::Not,
                _ => a::UnOp::Neg,
            };
            Ok(a::Expr::Unary(span_of(cst), un, Box::new(e2)))
        }
        CstExpr::Cast(e, _cc, ty) => {
            let e2 = lower_expr(e)?;
            Ok(a::Expr::Cast(
                span_of(cst),
                Box::new(e2),
                ty.span,
                token_text(ty),
            ))
        }
        CstExpr::Group(w) => lower_expr(&w.value),
        CstExpr::FnLam(_kw, params, ret, body) => {
            let body2 = lower_block(body)?;
            let ps: Vec<a::FnParam<RawName>> = params
                .value
                .iter()
                .flat_map(|sep| sep.iter())
                .map(lower_param)
                .collect();
            Ok(a::Expr::Lam(
                span_of(cst),
                ps,
                ret.as_ref().map(lower_ann),
                Box::new(body2),
            ))
        }
        CstExpr::If(_kw, cond, then_b, _else_kw, else_t) => {
            let cond2 = lower_expr(cond)?;
            let then2 = lower_block(then_b)?;
            let else2 = match &**else_t {
                CstElse::Block(b) => lower_block(b)?,
                CstElse::If(e) => lower_expr(e)?,
            };
            Ok(a::Expr::If(
                span_of(cst),
                Box::new(cond2),
                Box::new(then2),
                Box::new(else2),
            ))
        }
        CstExpr::Match(_kw, scrut, _o, arms, _c) => {
            let scrut2 = lower_expr(scrut)?;
            let mut arms2 = Vec::with_capacity(arms.len());
            for arm in arms {
                arms2.push(lower_arm(arm)?);
            }
            Ok(a::Expr::Match(span_of(cst), Box::new(scrut2), arms2))
        }
        CstExpr::List(w) => {
            let sp = span_of(cst);
            match &w.value {
                None => Ok(a::Expr::List(sp, Vec::new(), None)),
                Some(inner) => {
                    let CstListExpr { elems, spread } = &**inner;
                    let mut es = Vec::with_capacity(elems.len());
                    for e in elems.iter() {
                        es.push(lower_expr(e)?);
                    }
                    let tl = match spread {
                        Some((_c, _dd, e)) => Some(Box::new(lower_expr(e)?)),
                        None => None,
                    };
                    Ok(a::Expr::List(sp, es, tl))
                }
            }
        }
        CstExpr::Block(b) => lower_block(b),
        CstExpr::Try(e, q) => {
            let e2 = lower_expr(e)?;
            Ok(a::Expr::Try(span_of(cst), q.span, Box::new(e2)))
        }
        CstExpr::Interp(begin, parts) => lower_interp(span_of(cst), begin, parts),
        CstExpr::Jsx(j) => Ok(a::Expr::Jsx(span_of(cst), lower_jsx(j)?)),
    }
}

fn lower_interp(sp: Span, begin: &Token, parts: &[(CstExpr, Token)]) -> Lower<a::Expr<RawName>> {
    // Empty chunks vanish here: "a${x}" is just "a" ++ x.
    fn chunk_of(t: &Token) -> &str {
        match &t.kind {
            TokenKind::StringBegin(s) | TokenKind::StringMid(s) | TokenKind::StringEnd(s) => s,
            k => maxim_panic!("lower_interp: not an interpolation chunk {k:?}"),
        }
    }
    fn str_part(t: &Token, out: &mut Vec<a::InterpPart<RawName>>) {
        let c = chunk_of(t);
        if !c.is_empty() {
            out.push(a::InterpPart::Str(c.to_string()));
        }
    }
    let mut out = Vec::new();
    str_part(begin, &mut out);
    for (e, delim) in parts {
        out.push(a::InterpPart::Expr(lower_expr(e)?));
        str_part(delim, &mut out);
    }
    Ok(a::Expr::Interp(sp, out))
}

// JSX --------------------------------------------------------------------------

fn lower_jsx(cst: &CstJsx) -> Lower<a::JsxElem<RawName>> {
    fn tag_name_of(t: &Token) -> &str {
        match &t.kind {
            TokenKind::JsxTagStart(n) => n,
            k => maxim_panic!("lower_jsx: not a tag start {k:?}"),
        }
    }
    fn close_name_of(t: &Token) -> &str {
        match &t.kind {
            TokenKind::JsxClose(n) => n,
            k => maxim_panic!("lower_jsx: not a closing tag {k:?}"),
        }
    }

    let element = |open: &Token,
                   attrs: &[CstJsxAttr],
                   children: &[CstJsxChild],
                   has_close: bool|
     -> Lower<a::JsxElem<RawName>> {
        let name = tag_name_of(open);
        let tag = if name.chars().next().is_some_and(|c| c.is_uppercase()) {
            a::JsxTag::Component(open.span, RawName::plain(name))
        } else {
            a::JsxTag::Intrinsic(open.span, name.to_string())
        };
        let mut attrs2 = Vec::with_capacity(attrs.len());
        for attr in attrs {
            attrs2.push(lower_jsx_attr(&tag, attr)?);
        }
        check_dup_attrs(&attrs2)?;
        let mut children2 = Vec::new();
        for c in children {
            children2.extend(lower_jsx_child(c)?);
        }
        Ok(a::JsxElem {
            span: span_of(cst),
            tag,
            attrs: attrs2,
            children: children2,
            has_close,
        })
    };

    match cst {
        CstJsx::SelfClose(open, attrs, _close) => element(open, attrs, &[], false),
        CstJsx::Element(open, attrs, _gt, children, close) => {
            if close_name_of(close) != tag_name_of(open) {
                Err(Diagnostic::error(
                    "E1113",
                    "MISMATCHED CLOSING TAG",
                    close.span,
                    format!(
                        "this closes `{}`, but the open element is `{}`",
                        close_name_of(close),
                        tag_name_of(open)
                    ),
                )
                .with_secondary(vec![(open.span, "the element opens here".to_string())])
                .with_hints(vec![format!(
                    "Every element closes with its own tag: `</{}>`.",
                    tag_name_of(open)
                )]))
            } else {
                element(open, attrs, children, true)
            }
        }
    }
}

fn lower_jsx_attr(tag: &a::JsxTag<RawName>, attr: &CstJsxAttr) -> Lower<a::JsxAttr<RawName>> {
    fn attr_name_of(t: &Token) -> &str {
        match &t.kind {
            TokenKind::JsxAttrName(n) => n,
            k => maxim_panic!("lower_jsx_attr: not an attribute name {k:?}"),
        }
    }
    let (name_tok, value) = match attr {
        CstJsxAttr::Bare(t) => (t, None),
        CstJsxAttr::Value(t, _, CstJsxAttrValue::Str(v)) => {
            (t, Some(a::Expr::Lit(v.span, lit_of(v))))
        }
        CstJsxAttr::Value(t, _, CstJsxAttrValue::Interp(begin, parts)) => {
            let parts_span = {
                let delims: Vec<&Token> = parts.iter().map(|(_, d)| d).collect();
                span_of(&delims)
            };
            (
                t,
                Some(lower_interp(begin.span.merge(parts_span), begin, parts)?),
            )
        }
        CstJsxAttr::Value(t, _, CstJsxAttrValue::Expr(w)) => (t, Some(lower_expr(&w.value)?)),
    };
    let name = attr_name_of(name_tok);
    let sp = name_tok.span;
    let on_component = matches!(tag, a::JsxTag::Component(_, _));
    let segments: Vec<&str> = name.split(':').collect();
    match &segments[..] {
        // On a component tag, `on:` names an OUT event (checked against
        // the component's `Out` type by the checker — E1115 there); on an
        // intrinsic element it is a DOM event.
        ["on", event] => match value {
            Some(v) => Ok(a::JsxAttr::On(sp, event.to_string(), v)),
            None => Err(Diagnostic::error(
                "E1117",
                "EVENT NEEDS A HANDLER",
                sp,
                format!("`{name}` needs a value"),
            )
            .with_hints(vec![format!("Write `{name}={{message}}`.")])),
        },
        _ if on_component && (value.is_none() || name.contains(':')) => {
            let plain_name: &str = name.split(':').next().unwrap_or(name);
            Err(Diagnostic::error(
                "E1116",
                "INVALID COMPONENT ATTRIBUTE",
                sp,
                if name.contains(':') {
                    format!("`{name}` is not a parameter name")
                } else {
                    format!("`{name}` needs a value on a component")
                },
            )
            .with_hints(vec![format!(
                "Component attributes are named arguments: `{plain_name}={{value}}`."
            )]))
        }
        _ => Ok(a::JsxAttr::Attr(sp, name.to_string(), value)),
    }
}

fn check_dup_attrs(attrs: &[a::JsxAttr<RawName>]) -> Lower<()> {
    let key = |a: &a::JsxAttr<RawName>| -> String {
        match a {
            a::JsxAttr::Attr(_, n, _) => n.clone(),
            a::JsxAttr::On(_, ev, _) => format!("on:{ev}"),
        }
    };
    let mut seen: Vec<String> = Vec::new();
    for attr in attrs {
        let k = key(attr);
        if seen.contains(&k) {
            return Err(Diagnostic::error(
                "E1114",
                "DUPLICATE ATTRIBUTE",
                attr.span(),
                format!("`{k}` appears twice on this element"),
            ));
        }
        seen.push(k);
    }
    Ok(())
}

fn lower_jsx_child(c: &CstJsxChild) -> Lower<Vec<a::JsxChild<RawName>>> {
    match c {
        CstJsxChild::Text(t) => match &t.kind {
            TokenKind::JsxText(raw_text) => Ok(cook_jsx_text(raw_text)
                .map(|cooked| a::JsxChild::Text(t.span, cooked))
                .into_iter()
                .collect()),
            k => maxim_panic!("lower_jsx_child: not element text {k:?}"),
        },
        CstJsxChild::Elem(j) => Ok(vec![a::JsxChild::Elem(lower_jsx(j)?)]),
        CstJsxChild::Splice(w) => {
            let e = lower_expr(&w.value)?;
            Ok(vec![a::JsxChild::Splice(span_of(w), e)])
        }
        CstJsxChild::Spread(_o, _dd, e, _c) => {
            let e2 = lower_expr(e)?;
            Ok(vec![a::JsxChild::Spread(span_of(c), e2)])
        }
        CstJsxChild::Hole(_o, e, _c) => {
            let e2 = lower_expr(e)?;
            Ok(vec![a::JsxChild::Hole(span_of(c), e2)])
        }
    }
}

/// The JSX whitespace rule (the CST keeps text verbatim; this applies at
/// lowering): a run that is only whitespace and contains a newline
/// vanishes; leading/trailing whitespace containing a newline is
/// stripped; interior whitespace containing a newline collapses to one
/// space. Same-line whitespace always survives.
fn cook_jsx_text(t: &str) -> Option<String> {
    fn is_ws(c: char) -> bool {
        c == ' ' || c == '\t' || c == '\n' || c == '\r'
    }
    let is_ws_run = |s: &str| s.chars().all(is_ws);
    let has_nl = |s: &str| s.contains('\n');
    if t.is_empty() {
        return None;
    }
    if is_ws_run(t) && has_nl(t) {
        return None;
    }
    // Group into maximal runs of whitespace / non-whitespace.
    let mut runs: Vec<String> = Vec::new();
    for ch in t.chars() {
        match runs.last_mut() {
            Some(last) if last.chars().next().map(is_ws) == Some(is_ws(ch)) => last.push(ch),
            _ => runs.push(ch.to_string()),
        }
    }
    let n = runs.len();
    let cooked: String = runs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if !is_ws_run(r) || !has_nl(r) {
                r.as_str()
            } else if i == 0 || i == n - 1 {
                ""
            } else {
                " "
            }
        })
        .collect();
    Some(cooked)
}

/// Strip the trailing `?`s off a pipeline stage (direct only — a
/// `Group` blocks the peel), innermost first.
fn peel_try(e: &CstExpr) -> (&CstExpr, Vec<&Token>) {
    match e {
        CstExpr::Try(inner, q) => {
            let (stage, mut qs) = peel_try(inner);
            qs.push(q);
            (stage, qs)
        }
        e => (e, Vec::new()),
    }
}

fn lower_arm(arm: &CstArm) -> Lower<a::Arm<RawName>> {
    let CstArm(p, guard, _arrow, body) = arm;
    let p2 = lower_pat(p)?;
    let guard2 = match guard {
        Some((_kw, g)) => Some(lower_expr(g)?),
        None => None,
    };
    let body2 = lower_expr(body)?;
    Ok(a::Arm {
        span: span_of(arm),
        pat: p2,
        guard: guard2,
        body: body2,
    })
}

fn lower_arg(arg: &CstArg) -> Lower<a::Arg<RawName>> {
    match arg {
        CstArg::Pos(e) => Ok(a::Arg::Pos(lower_expr(e)?)),
        CstArg::Lab(name, _colon, e) => {
            Ok(a::Arg::Lab(name.span, token_text(name), lower_expr(e)?))
        }
        CstArg::Spread(dd, _) => Err(Diagnostic::error(
            "E1107",
            "MISPLACED SPREAD",
            dd.span,
            "`..` must be the first argument of a record update",
        )
        .with_hints(vec![
            "The spread comes first: `User(..u, age: 31)`.".to_string(),
        ])),
    }
}

fn lower_update_field(arg: &CstArg) -> Lower<(Span, String, a::Expr<RawName>)> {
    match arg {
        CstArg::Lab(name, _colon, e) => {
            let e2 = lower_expr(e)?;
            Ok((name.span, token_text(name), e2))
        }
        CstArg::Pos(e) => Err(Diagnostic::error(
            "E1109",
            "UNLABELED UPDATE",
            span_of(&**e),
            "record updates are written `field: value`",
        )
        .with_hints(vec![
            "After `..base`, every update is labeled: `field: value`.".to_string(),
        ])),
        CstArg::Spread(dd, _) => Err(Diagnostic::error(
            "E1107",
            "MISPLACED SPREAD",
            dd.span,
            "a record update takes exactly one `..base`, written first",
        )),
    }
}

fn lower_pat(cst: &CstPat) -> Lower<a::Pat<RawName>> {
    match cst {
        CstPat::Wild(t) => Ok(a::Pat::Wild(t.span)),
        CstPat::Var(t) => Ok(a::Pat::Var(t.span, raw(t))),
        CstPat::Lit(t) => Ok(a::Pat::Lit(t.span, lit_of(t))),
        CstPat::Ctor(qual, name, args) => {
            let mut args2 = Vec::new();
            if let Some(w) = args {
                for p in w.value.iter() {
                    args2.push(lower_pat(p)?);
                }
            }
            let q = qual.as_ref().map(|(qt, _dot)| token_text(qt));
            Ok(a::Pat::Ctor(
                span_of(cst),
                RawName {
                    qualifier: q,
                    base: token_text(name),
                },
                args2,
            ))
        }
        CstPat::List(w) => match &w.value {
            None => Ok(a::Pat::List(span_of(cst), Vec::new(), None)),
            Some(inner) => {
                let CstListPat { elems, rest } = &**inner;
                let mut ps = Vec::with_capacity(elems.len());
                for p in elems.iter() {
                    ps.push(lower_pat(p)?);
                }
                let rest2 = rest.as_ref().map(|(_c, _dd, b)| a::PatRest {
                    span: b.span,
                    name: binder_of(b),
                });
                Ok(a::Pat::List(span_of(cst), ps, rest2))
            }
        },
    }
}

// Helpers ---------------------------------------------------------------------

fn token_text(t: &Token) -> String {
    match &t.kind {
        TokenKind::Lower(txt) | TokenKind::Upper(txt) => txt.clone(),
        TokenKind::Hole => "_".to_string(),
        _ => t.text.clone(),
    }
}

fn raw(t: &Token) -> RawName {
    RawName::plain(token_text(t))
}

fn binder_of(t: &Token) -> Option<RawName> {
    match t.kind {
        TokenKind::Hole => None,
        _ => Some(raw(t)),
    }
}

fn lit_of(t: &Token) -> a::Lit {
    match &t.kind {
        TokenKind::Int(n) => a::Lit::Int(n.clone()),
        TokenKind::Float(d) => a::Lit::Float(*d),
        TokenKind::Str(s) => a::Lit::Str(s.clone()),
        k => maxim_panic!("lit_of: not a literal token {k:?}"),
    }
}

fn bin_of(o: Operator) -> a::BinOp {
    match o {
        Operator::Add => a::BinOp::Add,
        Operator::Sub => a::BinOp::Sub,
        Operator::Mul => a::BinOp::Mul,
        Operator::Div => a::BinOp::Div,
        Operator::IntDiv => a::BinOp::IntDiv,
        Operator::Mod => a::BinOp::Mod,
        Operator::Pow => a::BinOp::Pow,
        Operator::Concat => a::BinOp::Concat,
        Operator::Eq => a::BinOp::Eq,
        Operator::Neq => a::BinOp::Neq,
        Operator::Le => a::BinOp::Le,
        Operator::Ge => a::BinOp::Ge,
        Operator::And => a::BinOp::And,
        Operator::Or => a::BinOp::Or,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_module;
    use crate::span::FileId;

    const FID: FileId = FileId(0);

    fn lower_ok(src: &str) -> Vec<a::Decl<RawName>> {
        let m = parse_module(FID, src).expect("parses");
        lower_module(&m).expect("lowers").decls
    }

    fn expect_lower_code(src: &str, code: &str) {
        let m = parse_module(FID, src).expect("parses");
        match lower_module(&m) {
            Err(d) => assert_eq!(d.code.0, code),
            Ok(_) => panic!("expected a lowering error"),
        }
    }

    // From ParserTests.hs errorTests (lowering half).

    #[test]
    fn spread_after_other_arguments() {
        expect_lower_code("module M\n\nlet x = f(1, ..u)\n", "E1107");
    }

    #[test]
    fn update_on_a_non_constructor() {
        expect_lower_code("module M\n\nlet x = f(..u, age: 1)\n", "E1108");
    }

    #[test]
    fn positional_argument_in_an_update() {
        expect_lower_code("module M\n\nlet x = User(..u, 5)\n", "E1109");
    }

    // From ParserTests.hs tryTests (lowering half).

    #[test]
    fn a_piped_stages_try_hoists_over_the_pipe() {
        match &lower_ok("module M\n\nlet a = xs |> parse?\n")[..] {
            [a::Decl::Let(_, _, _, a::Expr::Try(_, _, inner))] => {
                assert!(matches!(**inner, a::Expr::Pipe(_, _, _, _)));
            }
            ds => panic!("expected ETry(EPipe …), got {ds:?}"),
        }
    }

    #[test]
    fn a_parenthesized_try_does_not_hoist() {
        match &lower_ok("module M\n\nlet a = xs |> (h?)\n")[..] {
            [a::Decl::Let(_, _, _, a::Expr::Pipe(_, _, _, r))] => {
                assert!(matches!(**r, a::Expr::Try(_, _, _)));
            }
            ds => panic!("expected EPipe(_, ETry …), got {ds:?}"),
        }
    }

    #[test]
    fn double_try_hoists_both_layers() {
        match &lower_ok("module M\n\nlet a = xs |> parse??\n")[..] {
            [a::Decl::Let(_, _, _, a::Expr::Try(_, _, inner))] => match &**inner {
                a::Expr::Try(_, _, inner2) => {
                    assert!(matches!(**inner2, a::Expr::Pipe(_, _, _, _)));
                }
                e => panic!("expected ETry(ETry(EPipe …)), got {e:?}"),
            },
            ds => panic!("expected ETry(ETry(EPipe …)), got {ds:?}"),
        }
    }

    // From ParserTests.hs interpParserTests (lowering half).

    #[test]
    fn empty_chunks_are_elided_in_lowering() {
        match &lower_ok("module M\n\nlet a = \"${x}\"\n")[..] {
            [a::Decl::Let(_, _, _, a::Expr::Interp(_, parts))] => match &parts[..] {
                [a::InterpPart::Expr(_)] => {}
                ps => panic!("expected a single IPExpr, got {ps:?}"),
            },
            ds => panic!("unexpected shape: {ds:?}"),
        }
    }

    #[test]
    fn chunks_and_splices_interleave_in_order() {
        match &lower_ok("module M\n\nlet a = \"a${x}b\"\n")[..] {
            [a::Decl::Let(_, _, _, a::Expr::Interp(_, parts))] => match &parts[..] {
                [
                    a::InterpPart::Str(s1),
                    a::InterpPart::Expr(_),
                    a::InterpPart::Str(s2),
                ] => {
                    assert_eq!(s1, "a");
                    assert_eq!(s2, "b");
                }
                ps => panic!("unexpected shape: {ps:?}"),
            },
            ds => panic!("unexpected shape: {ds:?}"),
        }
    }

    // Phase-4 gate: every parseable repo .mx file lowers without a panic.

    #[test]
    fn repo_corpus_lowers_clean() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../language-project");
        let mut lowered = 0;
        for dir in ["stdlib", "examples", "compiler/test/fixtures"] {
            let mut stack = vec![std::path::PathBuf::from(root).join(dir)];
            while let Some(d) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&d) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|e| e == "mx") {
                        let src = std::fs::read_to_string(&p).unwrap();
                        if let Ok(m) = parse_module(FID, &src) {
                            // Lowering may report a diagnostic (some
                            // fixtures are E11xx cases) but must not panic.
                            if lower_module(&m).is_ok() {
                                lowered += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(lowered >= 10, "corpus should lower, got {lowered}");
    }
}
