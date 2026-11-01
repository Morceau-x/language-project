//! The renamer: resolve every `RawName` to a unique `Name`, in two passes
//! per module — collect the headers (top-level definitions, duplicate
//! checks), then resolve bodies walking lexical scopes.
//!
//! Multiple errors are collected in one run; a failed resolution yields a
//! `NameSort::Error` name so the type checker can suppress cascades.

use std::collections::BTreeMap;

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::maxim_panic;
use crate::names::{ModuleExports, ModuleName, Name, NameSort, RawName};
use crate::span::Span;
use crate::unique::UniqueSupply;

#[derive(Clone, Debug, Default)]
pub struct RenameEnv {
    /// Modules in scope without an `import`: the stdlib and the virtual
    /// intrinsic modules.
    pub auto_modules: BTreeMap<ModuleName, ModuleExports>,
    /// Modules that require an explicit `import`.
    pub user_modules: BTreeMap<ModuleName, ModuleExports>,
    /// Constructors exposed unqualified: `Some None Ok Err True False`.
    pub prelude_ctors: BTreeMap<String, Name>,
    /// Types in scope unqualified: `Int Float String Bool List Option Result`.
    pub prelude_types: BTreeMap<String, Name>,
}

/// Returns the renamed module, what it exports to importers, and the
/// next unique seed (thread it into the next module's renaming — uniques
/// key the cross-module type environment, so they must never collide).
pub fn rename_module(
    env: &RenameEnv,
    seed: i64,
    m: &Module<RawName>,
) -> Result<(Module<Name>, ModuleExports, i64), Vec<Diagnostic>> {
    let mut rn = Rn {
        supply: UniqueSupply::from_value(seed),
        diags: Vec::new(),
    };
    let (renamed, exports) = do_rename(&mut rn, env, m);
    if rn.diags.is_empty() {
        Ok((renamed, exports, rn.supply.value()))
    } else {
        Err(rn.diags)
    }
}

// Machinery -------------------------------------------------------------------

struct Rn {
    supply: UniqueSupply,
    diags: Vec<Diagnostic>,
}

impl Rn {
    fn fresh(&mut self, txt: &str, sort: NameSort, sp: Span) -> Name {
        Name {
            unique: self.supply.fresh(),
            text: txt.to_string(),
            sort,
            def_span: sp,
        }
    }

    fn report(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    fn error_name(&mut self, txt: &str, sp: Span) -> Name {
        self.fresh(txt, NameSort::Error, sp)
    }
}

#[derive(Default)]
struct TopDefs {
    values: BTreeMap<String, Name>,
    fns: BTreeMap<String, Name>,
    ctors: BTreeMap<String, Name>,
    types: BTreeMap<String, Name>,
    /// Synthesized cast names (`floatToCelsius`), keyed by the same
    /// text. NOT in the value namespace — casts aren't first-class —
    /// but they claim the emitted-JS name, so both directions collide.
    casts: BTreeMap<String, Name>,
    /// `component` declarations — their own namespace (uppercase JSX
    /// tags resolve here), but they claim an emitted-JS const, so they
    /// collide with constructors of the same text.
    components: BTreeMap<String, Name>,
}

#[derive(Clone)]
struct Ctx<'a> {
    quals: &'a BTreeMap<String, ModuleExports>,
    tops: &'a TopDefs,
    env: &'a RenameEnv,
    /// Top values visible at this point (all of them inside `fn` bodies;
    /// fns plus earlier lets inside top-level `let` bodies).
    top_visible: BTreeMap<String, Name>,
    /// Lexical scopes, innermost LAST.
    scopes: Vec<BTreeMap<String, Name>>,
}

impl<'a> Ctx<'a> {
    fn push_scope(&self, s: BTreeMap<String, Name>) -> Ctx<'a> {
        let mut c = self.clone();
        c.scopes.push(s);
        c
    }

    fn with_top_visible(&self, tv: BTreeMap<String, Name>) -> Ctx<'a> {
        let mut c = self.clone();
        c.top_visible = tv;
        c
    }
}

// The renamer ------------------------------------------------------------------

fn do_rename(rn: &mut Rn, env: &RenameEnv, m: &Module<RawName>) -> (Module<Name>, ModuleExports) {
    let mut import_quals: BTreeMap<String, ModuleExports> = BTreeMap::new();
    for imp in &m.imports {
        match env.user_modules.get(&imp.module) {
            Some(ex) => {
                import_quals.insert(imp.module.0.clone(), ex.clone());
            }
            None => {
                if !env.auto_modules.contains_key(&imp.module) {
                    rn.report(unknown_module_diag(
                        imp.span,
                        &imp.module.0,
                        &module_candidates(env),
                    ));
                }
                // redundant import of an auto module is harmless
            }
        }
    }
    // Left-biased union: imports win over auto modules.
    let mut quals: BTreeMap<String, ModuleExports> = env
        .auto_modules
        .iter()
        .map(|(k, v)| (k.0.clone(), v.clone()))
        .collect();
    for (k, v) in import_quals {
        quals.insert(k, v);
    }

    let tops = collect_tops(rn, &m.name, &m.decls);
    let ctx0 = Ctx {
        quals: &quals,
        tops: &tops,
        env,
        top_visible: BTreeMap::new(),
        scopes: Vec::new(),
    };

    let mut decls: Vec<Decl<Name>> = Vec::with_capacity(m.decls.len());
    let mut visible_lets: BTreeMap<String, Name> = BTreeMap::new();
    for d in &m.decls {
        match d {
            Decl::Type(td) => {
                decls.push(Decl::Type(rename_type_decl(&tops, td)));
            }
            Decl::Fn(fnd) => {
                let ctx = ctx0.with_top_visible(tops.values.clone());
                decls.push(Decl::Fn(rename_fn(rn, &ctx, fnd)));
            }
            Decl::Let(sp, r, ann, e) => {
                // Fns plus earlier lets are visible; the let itself is not.
                let mut visible = tops.fns.clone();
                for (k, v) in &visible_lets {
                    visible.insert(k.clone(), v.clone());
                }
                let ctx = ctx0.with_top_visible(visible);
                let e2 = rename_expr(rn, &ctx, e);
                // Always present: pass 1 inserted it (on duplicates, the
                // first wins).
                let name = tops.values.get(&r.base).cloned().unwrap_or_else(|| {
                    maxim_panic!("renamer: top-level value missing from pass 1")
                });
                visible_lets.insert(r.base.clone(), name.clone());
                decls.push(Decl::Let(*sp, name, ann.clone(), e2));
            }
            Decl::Cast(cd) => {
                let ctx = ctx0.with_top_visible(tops.values.clone());
                decls.push(Decl::Cast(rename_cast(rn, &ctx, cd)));
            }
            Decl::Component(cd) => {
                let ctx = ctx0.with_top_visible(tops.values.clone());
                decls.push(Decl::Component(rename_component(rn, &ctx, cd)));
            }
        }
    }

    let exports = ModuleExports {
        values: tops.values.clone(),
        ctors: tops.ctors.clone(),
        types: tops.types.clone(),
        components: tops.components.clone(),
    };
    (
        Module {
            name: m.name.clone(),
            name_span: m.name_span,
            imports: m.imports.clone(),
            decls,
        },
        exports,
    )
}

fn collect_tops(rn: &mut Rn, mn: &ModuleName, decls: &[Decl<RawName>]) -> TopDefs {
    let mut tops = TopDefs::default();

    fn insert_checked(rn: &mut Rn, what: &str, m: &mut BTreeMap<String, Name>, n: Name) {
        match m.get(&n.text) {
            None => {
                m.insert(n.text.clone(), n);
            }
            Some(prev) => {
                rn.report(
                    Diagnostic::error(
                        "E2004",
                        "DUPLICATE DEFINITION",
                        n.def_span,
                        format!(
                            "there is already a {what} named `{}` in this module",
                            n.text
                        ),
                    )
                    .with_secondary(vec![(
                        prev.def_span,
                        "it was first defined here".to_string(),
                    )]),
                );
            }
        }
    }

    // A value and a cast may not share an emitted name; the check runs
    // in both directions.
    fn check_against_casts(rn: &mut Rn, tops: &TopDefs, n: &Name) {
        if let Some(prev) = tops.casts.get(&n.text) {
            rn.report(
                Diagnostic::error(
                    "E2004",
                    "DUPLICATE DEFINITION",
                    n.def_span,
                    format!("a cast in this module already emits as `{}`", n.text),
                )
                .with_secondary(vec![(
                    prev.def_span,
                    "the cast is declared here".to_string(),
                )]),
            );
        }
    }

    for d in decls {
        match d {
            Decl::Fn(FnDef { span, name: r, .. }) => {
                let n = rn.fresh(&r.base, NameSort::TopVal(mn.clone()), *span);
                check_against_casts(rn, &tops, &n);
                insert_checked(rn, "definition", &mut tops.values, n.clone());
                tops.fns.insert(r.base.clone(), n);
            }
            Decl::Let(sp, r, _, _) => {
                let n = rn.fresh(&r.base, NameSort::TopVal(mn.clone()), *sp);
                check_against_casts(rn, &tops, &n);
                insert_checked(rn, "definition", &mut tops.values, n);
            }
            Decl::Cast(cd) => {
                let n = rn.fresh(&cd.name.base, NameSort::TopVal(mn.clone()), cd.span);
                // Cast-vs-cast collisions are always the SAME (source,
                // target) pair (the synthesized text is determined by it)
                // — the checker reports those as E3030 DUPLICATE CAST,
                // with better wording.
                if let Some(prev) = tops.values.get(&n.text) {
                    rn.report(
                        Diagnostic::error(
                            "E2004",
                            "DUPLICATE DEFINITION",
                            cd.span,
                            format!(
                                "this cast emits as `{}`, which is already defined in this module",
                                n.text
                            ),
                        )
                        .with_secondary(vec![(
                            prev.def_span,
                            "it was first defined here".to_string(),
                        )]),
                    );
                }
                tops.casts.entry(n.text.clone()).or_insert(n);
            }
            Decl::Component(cd) => {
                let n = rn.fresh(&cd.name.base, NameSort::Component(mn.clone()), cd.span);
                insert_checked(rn, "component", &mut tops.components, n);
            }
            Decl::Type(TypeDecl {
                span, name: r, rhs, ..
            }) => {
                let ty_n = rn.fresh(&r.base, NameSort::TyCon(mn.clone()), *span);
                insert_checked(rn, "type", &mut tops.types, ty_n);
                let ctor_names: Vec<Name> = match rhs {
                    TypeRhs::Variants(vs) => vs
                        .iter()
                        .map(|v| rn.fresh(&v.name.base, NameSort::Ctor(mn.clone()), v.span))
                        .collect(),
                    TypeRhs::Record(cr, _) => {
                        vec![rn.fresh(&cr.base, NameSort::Ctor(mn.clone()), *span)]
                    }
                };
                for n in ctor_names {
                    insert_checked(rn, "constructor", &mut tops.ctors, n);
                }
            }
        }
    }

    // A component's manifest const and a constructor share the module's
    // JS namespace; either declaration order must collide.
    let collisions: Vec<(String, Name, Name)> = tops
        .components
        .iter()
        .filter_map(|(txt, comp)| {
            tops.ctors
                .get(txt)
                .map(|ctor| (txt.clone(), comp.clone(), ctor.clone()))
        })
        .collect();
    for (txt, comp, ctor) in collisions {
        rn.report(
            Diagnostic::error(
                "E2004",
                "DUPLICATE DEFINITION",
                comp.def_span,
                format!("component `{txt}` collides with a constructor of the same name"),
            )
            .with_secondary(vec![(ctor.def_span, "the constructor is here".to_string())]),
        );
    }

    tops
}

fn rename_type_decl(tops: &TopDefs, td: &TypeDecl<RawName>) -> TypeDecl<Name> {
    let lookup_or = |m: &BTreeMap<String, Name>, raw: &RawName| -> Name {
        m.get(&raw.base)
            .cloned()
            .unwrap_or_else(|| maxim_panic!("renamer: type-level name missing from pass 1"))
    };
    TypeDecl {
        span: td.span,
        name: lookup_or(&tops.types, &td.name),
        params: td.params.clone(),
        rhs: match &td.rhs {
            TypeRhs::Variants(vs) => TypeRhs::Variants(
                vs.iter()
                    .map(|v| Variant {
                        span: v.span,
                        name: lookup_or(&tops.ctors, &v.name),
                        args: v.args.clone(),
                    })
                    .collect(),
            ),
            TypeRhs::Record(cr, fields) => {
                TypeRhs::Record(lookup_or(&tops.ctors, cr), fields.clone())
            }
        },
    }
}

fn rename_fn(rn: &mut Rn, ctx: &Ctx, fnd: &FnDef<RawName>) -> FnDef<Name> {
    let name = ctx
        .tops
        .values
        .get(&fnd.name.base)
        .cloned()
        .unwrap_or_else(|| maxim_panic!("renamer: fn missing from pass 1"));
    let mut params: Vec<FnParam<Name>> = Vec::with_capacity(fnd.params.len());
    let mut scope: BTreeMap<String, Name> = BTreeMap::new();
    for p in &fnd.params {
        match &p.name {
            None => params.push(FnParam {
                span: p.span,
                name: None,
                ann: p.ann.clone(),
            }),
            Some(r) => {
                let n = rn.fresh(&r.base, NameSort::Local, p.span);
                check_dup_binder(rn, "parameter", &mut scope, n.clone());
                params.push(FnParam {
                    span: p.span,
                    name: Some(n),
                    ann: p.ann.clone(),
                });
            }
        }
    }
    let body = rename_expr(rn, &ctx.push_scope(scope), &fnd.body);
    FnDef {
        span: fnd.span,
        name,
        params,
        ret: fnd.ret.clone(),
        body,
    }
}

/// Rename a `component` manifest: the name comes from pass 1, the wired
/// references resolve as ordinary top-level values.
fn rename_component(rn: &mut Rn, ctx: &Ctx, cd: &ComponentDef<RawName>) -> ComponentDef<Name> {
    let name = ctx
        .tops
        .components
        .get(&cd.name.base)
        .cloned()
        .unwrap_or_else(|| maxim_panic!("renamer: component missing from pass 1"));
    let mut wire =
        |w: &(Span, RawName)| -> (Span, Name) { (w.0, lookup_value(rn, ctx, w.0, &w.1.base)) };
    ComponentDef {
        span: cd.span,
        name,
        init: cd.init.as_ref().map(&mut wire),
        update: cd.update.as_ref().map(&mut wire),
        view: wire(&cd.view),
    }
}

fn rename_cast(rn: &mut Rn, ctx: &Ctx, cd: &CastDef<RawName>) -> CastDef<Name> {
    let name = ctx
        .tops
        .casts
        .get(&cd.name.base)
        .cloned()
        .unwrap_or_else(|| maxim_panic!("renamer: cast missing from pass 1"));
    let p = &cd.param;
    let (param, scope) = match &p.name {
        None => (
            FnParam {
                span: p.span,
                name: None,
                ann: p.ann.clone(),
            },
            BTreeMap::new(),
        ),
        Some(r) => {
            let n = rn.fresh(&r.base, NameSort::Local, p.span);
            let mut sc = BTreeMap::new();
            sc.insert(r.base.clone(), n.clone());
            (
                FnParam {
                    span: p.span,
                    name: Some(n),
                    ann: p.ann.clone(),
                },
                sc,
            )
        }
    };
    let body = rename_expr(rn, &ctx.push_scope(scope), &cd.body);
    CastDef {
        span: cd.span,
        name,
        src_head: cd.src_head.clone(),
        tgt_head: cd.tgt_head.clone(),
        param,
        ret: cd.ret.clone(),
        body,
    }
}

fn check_dup_binder(rn: &mut Rn, what: &str, sc: &mut BTreeMap<String, Name>, n: Name) {
    match sc.get(&n.text) {
        None => {
            sc.insert(n.text.clone(), n);
        }
        Some(prev) => {
            rn.report(
                Diagnostic::error(
                    "E2008",
                    "DUPLICATE BINDER",
                    n.def_span,
                    format!("`{}` is already bound in this {what}", n.text),
                )
                .with_secondary(vec![(prev.def_span, "first used here".to_string())]),
            );
        }
    }
}

// Expressions -----------------------------------------------------------------

fn rename_expr(rn: &mut Rn, ctx: &Ctx, expr: &Expr<RawName>) -> Expr<Name> {
    match expr {
        Expr::Var(sp, r) => Expr::Var(*sp, lookup_value(rn, ctx, *sp, &r.base)),
        Expr::Con(sp, r) => Expr::Con(*sp, lookup_ctor(rn, ctx, *sp, r)),
        Expr::Lit(sp, l) => Expr::Lit(*sp, l.clone()),
        Expr::Dot(sp, e, m_sp, member) => rename_dot(rn, ctx, *sp, e, *m_sp, member),
        Expr::Field(sp, e, m_sp, member) => Expr::Field(
            *sp,
            Box::new(rename_expr(rn, ctx, e)),
            *m_sp,
            member.clone(),
        ),
        Expr::Call(sp, f, args) => Expr::Call(
            *sp,
            Box::new(rename_expr(rn, ctx, f)),
            args.iter().map(|a| rename_arg(rn, ctx, a)).collect(),
        ),
        Expr::Update(sp, ctor, ctor_sp, base, fields) => {
            let ctor2 = lookup_ctor(rn, ctx, *ctor_sp, ctor);
            let base2 = rename_expr(rn, ctx, base);
            let fields2 = fields
                .iter()
                .map(|(fsp, lbl, e)| (*fsp, lbl.clone(), rename_expr(rn, ctx, e)))
                .collect();
            Expr::Update(*sp, ctor2, *ctor_sp, Box::new(base2), fields2)
        }
        Expr::Bin(sp, op, op_sp, l, r) => Expr::Bin(
            *sp,
            *op,
            *op_sp,
            Box::new(rename_expr(rn, ctx, l)),
            Box::new(rename_expr(rn, ctx, r)),
        ),
        Expr::Unary(sp, op, e) => Expr::Unary(*sp, *op, Box::new(rename_expr(rn, ctx, e))),
        Expr::Pipe(sp, style, l, r) => Expr::Pipe(
            *sp,
            *style,
            Box::new(rename_expr(rn, ctx, l)),
            Box::new(rename_expr(rn, ctx, r)),
        ),
        Expr::If(sp, c, t, e) => Expr::If(
            *sp,
            Box::new(rename_expr(rn, ctx, c)),
            Box::new(rename_expr(rn, ctx, t)),
            Box::new(rename_expr(rn, ctx, e)),
        ),
        Expr::Match(sp, scrut, arms) => Expr::Match(
            *sp,
            Box::new(rename_expr(rn, ctx, scrut)),
            arms.iter().map(|arm| rename_arm(rn, ctx, arm)).collect(),
        ),
        Expr::Lam(sp, params, ret, body) => {
            let mut params2: Vec<FnParam<Name>> = Vec::with_capacity(params.len());
            let mut scope: BTreeMap<String, Name> = BTreeMap::new();
            for p in params {
                match &p.name {
                    None => params2.push(FnParam {
                        span: p.span,
                        name: None,
                        ann: p.ann.clone(),
                    }),
                    Some(r) => {
                        let n = rn.fresh(&r.base, NameSort::Local, p.span);
                        check_dup_binder(rn, "lambda", &mut scope, n.clone());
                        params2.push(FnParam {
                            span: p.span,
                            name: Some(n),
                            ann: p.ann.clone(),
                        });
                    }
                }
            }
            let body2 = rename_expr(rn, &ctx.push_scope(scope), body);
            Expr::Lam(*sp, params2, ret.clone(), Box::new(body2))
        }
        Expr::List(sp, elems, spread) => Expr::List(
            *sp,
            elems.iter().map(|e| rename_expr(rn, ctx, e)).collect(),
            spread.as_ref().map(|e| Box::new(rename_expr(rn, ctx, e))),
        ),
        Expr::Block(sp, stmts, result) => {
            let mut ctx2 = ctx.clone();
            let mut stmts2 = Vec::with_capacity(stmts.len());
            for s in stmts {
                match s {
                    Stmt::Let(ssp, r, ann, e) => {
                        let e2 = rename_expr(rn, &ctx2, e);
                        let n = rn.fresh(&r.base, NameSort::Local, *ssp);
                        let mut single = BTreeMap::new();
                        single.insert(r.base.clone(), n.clone());
                        ctx2 = ctx2.push_scope(single);
                        stmts2.push(Stmt::Let(*ssp, n, ann.clone(), e2));
                    }
                    Stmt::Discard(e) => {
                        stmts2.push(Stmt::Discard(rename_expr(rn, &ctx2, e)));
                    }
                }
            }
            let result2 = rename_expr(rn, &ctx2, result);
            Expr::Block(*sp, stmts2, Box::new(result2))
        }
        Expr::Try(sp, q_sp, e) => Expr::Try(*sp, *q_sp, Box::new(rename_expr(rn, ctx, e))),
        Expr::Cast(sp, e, ty_sp, ty) => {
            Expr::Cast(*sp, Box::new(rename_expr(rn, ctx, e)), *ty_sp, ty.clone())
        }
        Expr::Interp(sp, parts) => Expr::Interp(
            *sp,
            parts
                .iter()
                .map(|p| match p {
                    InterpPart::Str(t) => InterpPart::Str(t.clone()),
                    InterpPart::Expr(e) => InterpPart::Expr(rename_expr(rn, ctx, e)),
                })
                .collect(),
        ),
        Expr::Jsx(sp, el) => Expr::Jsx(*sp, rename_jsx_elem(rn, ctx, el)),
        Expr::Instance(_, _, _, _) => {
            maxim_panic!("rename_expr: EInstance is synthesized by Core lowering")
        }
    }
}

/// Rename a JSX element. A capitalized tag resolves in the COMPONENT
/// namespace: this module's `component` declarations first, then a
/// unique hit among the imported modules' components. Zero hits is
/// E2003; more than one import offering the name is E2009.
fn rename_jsx_elem(rn: &mut Rn, ctx: &Ctx, el: &JsxElem<RawName>) -> JsxElem<Name> {
    let tag = match &el.tag {
        JsxTag::Intrinsic(tsp, t) => JsxTag::Intrinsic(*tsp, t.clone()),
        JsxTag::Component(tsp, raw) => {
            let tag_name = &raw.base;
            let local_hit = ctx.tops.components.get(tag_name);
            let mut import_hits: Vec<&Name> = Vec::new();
            for ex in ctx.quals.values() {
                if let Some(n) = ex.components.get(tag_name) {
                    if !import_hits.iter().any(|h| h.unique == n.unique) {
                        import_hits.push(n);
                    }
                }
            }
            match (local_hit, &import_hits[..]) {
                (Some(n), _) => JsxTag::Component(*tsp, n.clone()),
                (None, [n]) => JsxTag::Component(*tsp, (*n).clone()),
                (None, []) => {
                    rn.report(
                        Diagnostic::error(
                            "E2003",
                            "UNKNOWN COMPONENT",
                            *tsp,
                            format!("there is no component `{tag_name}` in scope"),
                        )
                        .with_hints(vec![
                            "A capitalized tag names a `component` declaration — declare one, or import the module that does."
                                .to_string(),
                        ]),
                    );
                    JsxTag::Component(*tsp, rn.error_name(tag_name, *tsp))
                }
                (None, hits) => {
                    let homes: Vec<String> = hits
                        .iter()
                        .filter_map(|n| match &n.sort {
                            NameSort::Component(m) => Some(m.0.clone()),
                            _ => None,
                        })
                        .collect();
                    rn.report(
                        Diagnostic::error(
                            "E2009",
                            "AMBIGUOUS COMPONENT",
                            *tsp,
                            format!(
                                "`{tag_name}` is a component in more than one imported module: {}",
                                homes.join(", ")
                            ),
                        )
                        .with_hints(vec![
                            "Rename one of the components — dotted tags don't exist yet, so the tag alone must decide."
                                .to_string(),
                        ]),
                    );
                    JsxTag::Component(*tsp, rn.error_name(tag_name, *tsp))
                }
            }
        }
    };
    let attrs = el
        .attrs
        .iter()
        .map(|a| match a {
            JsxAttr::Attr(asp, n, v) => {
                JsxAttr::Attr(*asp, n.clone(), v.as_ref().map(|e| rename_expr(rn, ctx, e)))
            }
            JsxAttr::On(asp, ev, e) => JsxAttr::On(*asp, ev.clone(), rename_expr(rn, ctx, e)),
        })
        .collect();
    let children = el
        .children
        .iter()
        .map(|c| match c {
            JsxChild::Text(csp, t) => JsxChild::Text(*csp, t.clone()),
            JsxChild::Elem(el2) => JsxChild::Elem(rename_jsx_elem(rn, ctx, el2)),
            JsxChild::Splice(csp, e) => JsxChild::Splice(*csp, rename_expr(rn, ctx, e)),
            JsxChild::Spread(csp, e) => JsxChild::Spread(*csp, rename_expr(rn, ctx, e)),
            JsxChild::Hole(csp, e) => JsxChild::Hole(*csp, rename_expr(rn, ctx, e)),
        })
        .collect();
    JsxElem {
        span: el.span,
        tag,
        attrs,
        children,
        has_close: el.has_close,
    }
}

fn rename_arg(rn: &mut Rn, ctx: &Ctx, arg: &Arg<RawName>) -> Arg<Name> {
    match arg {
        Arg::Pos(e) => Arg::Pos(rename_expr(rn, ctx, e)),
        Arg::Lab(sp, lbl, e) => Arg::Lab(*sp, lbl.clone(), rename_expr(rn, ctx, e)),
    }
}

fn rename_arm(rn: &mut Rn, ctx: &Ctx, arm: &Arm<RawName>) -> Arm<Name> {
    let mut binders: BTreeMap<String, Name> = BTreeMap::new();
    let pat = rename_pat(rn, ctx, &mut binders, &arm.pat);
    let ctx2 = ctx.push_scope(binders);
    let guard = arm.guard.as_ref().map(|g| rename_expr(rn, &ctx2, g));
    let body = rename_expr(rn, &ctx2, &arm.body);
    Arm {
        span: arm.span,
        pat,
        guard,
        body,
    }
}

fn rename_pat(
    rn: &mut Rn,
    ctx: &Ctx,
    binders: &mut BTreeMap<String, Name>,
    pat: &Pat<RawName>,
) -> Pat<Name> {
    match pat {
        Pat::Wild(sp) => Pat::Wild(*sp),
        Pat::Var(sp, r) => {
            let n = rn.fresh(&r.base, NameSort::Local, *sp);
            check_dup_binder(rn, "pattern", binders, n.clone());
            Pat::Var(*sp, n)
        }
        Pat::Lit(sp, l) => Pat::Lit(*sp, l.clone()),
        Pat::Ctor(sp, r, args) => {
            let ctor = lookup_ctor(rn, ctx, *sp, r);
            let args2 = args
                .iter()
                .map(|p| rename_pat(rn, ctx, binders, p))
                .collect();
            Pat::Ctor(*sp, ctor, args2)
        }
        Pat::List(sp, elems, rest) => {
            let elems2 = elems
                .iter()
                .map(|p| rename_pat(rn, ctx, binders, p))
                .collect();
            let rest2 = rest.as_ref().map(|pr| match &pr.name {
                None => PatRest {
                    span: pr.span,
                    name: None,
                },
                Some(r) => {
                    let n = rn.fresh(&r.base, NameSort::Local, pr.span);
                    check_dup_binder(rn, "pattern", binders, n.clone());
                    PatRest {
                        span: pr.span,
                        name: Some(n),
                    }
                }
            });
            Pat::List(*sp, elems2, rest2)
        }
    }
}

// Resolution ------------------------------------------------------------------

fn lookup_value(rn: &mut Rn, ctx: &Ctx, sp: Span, txt: &str) -> Name {
    // Lexical scopes, innermost first.
    for s in ctx.scopes.iter().rev() {
        if let Some(n) = s.get(txt) {
            return n.clone();
        }
    }
    if let Some(n) = ctx.top_visible.get(txt) {
        return n.clone();
    }
    if let Some(later) = ctx.tops.values.get(txt) {
        rn.report(
            Diagnostic::error(
                "E2006",
                "USED BEFORE DEFINED",
                sp,
                format!("`{txt}` is defined later in this module"),
            )
            .with_secondary(vec![(
                later.def_span,
                "it is defined down here".to_string(),
            )])
            .with_hints(vec![
                "Top-level `let`s run top to bottom; `fn` definitions can be used anywhere."
                    .to_string(),
            ]),
        );
        return rn.error_name(txt, sp);
    }
    rn.report(unbound_diag(
        "E2001",
        "value",
        sp,
        txt,
        &value_candidates(ctx),
    ));
    rn.error_name(txt, sp)
}

fn lookup_ctor(rn: &mut Rn, ctx: &Ctx, sp: Span, r: &RawName) -> Name {
    match &r.qualifier {
        Some(q) => match ctx.quals.get(q) {
            Some(ex) => match ex.ctors.get(&r.base) {
                Some(n) => n.clone(),
                None => {
                    let cands: Vec<String> = ex.ctors.keys().cloned().collect();
                    rn.report(no_member_diag(sp, q, &r.base, &cands));
                    rn.error_name(&r.base, sp)
                }
            },
            None => {
                report_unknown_qualifier(rn, ctx, sp, q);
                rn.error_name(&r.base, sp)
            }
        },
        None => {
            if let Some(n) = ctx.tops.ctors.get(&r.base) {
                return n.clone();
            }
            if let Some(n) = ctx.env.prelude_ctors.get(&r.base) {
                return n.clone();
            }
            let mut d = unbound_diag("E2005", "constructor", sp, &r.base, &ctor_candidates(ctx));
            if ctx.quals.contains_key(&r.base) {
                d = d.with_hints(vec![format!(
                    "`{0}` is a module; pick one of its members, like `{0}.something`.",
                    r.base
                )]);
            }
            rn.report(d);
            rn.error_name(&r.base, sp)
        }
    }
}

fn rename_dot(
    rn: &mut Rn,
    ctx: &Ctx,
    sp: Span,
    e: &Expr<RawName>,
    m_sp: Span,
    member: &str,
) -> Expr<Name> {
    if let Expr::Con(
        _,
        RawName {
            qualifier: None,
            base: q,
        },
    ) = e
    {
        if let Some(ex) = ctx.quals.get(q) {
            return if is_upper_text(member) {
                match ex.ctors.get(member) {
                    Some(n) => Expr::Con(sp, n.clone()),
                    None => {
                        let cands: Vec<String> = ex.ctors.keys().cloned().collect();
                        rn.report(no_member_diag(m_sp, q, member, &cands));
                        Expr::Var(sp, rn.error_name(member, m_sp))
                    }
                }
            } else {
                match ex.values.get(member) {
                    Some(n) => Expr::Var(sp, n.clone()),
                    None => {
                        let cands: Vec<String> = ex.values.keys().cloned().collect();
                        rn.report(no_member_diag(m_sp, q, member, &cands));
                        Expr::Var(sp, rn.error_name(member, m_sp))
                    }
                }
            };
        }
        if !ctx.tops.ctors.contains_key(q) && !ctx.env.prelude_ctors.contains_key(q) {
            report_unknown_qualifier(rn, ctx, e.span(), q);
            return Expr::Var(sp, rn.error_name(member, m_sp));
        }
    }
    let e2 = rename_expr(rn, ctx, e);
    Expr::Field(sp, Box::new(e2), m_sp, member.to_string())
}

fn report_unknown_qualifier(rn: &mut Rn, ctx: &Ctx, sp: Span, q: &str) {
    let importable = ctx
        .env
        .user_modules
        .contains_key(&ModuleName(q.to_string()));
    let hints = if importable {
        vec![format!("Add `import {q}` at the top of this module.")]
    } else {
        suggestion_hints(q, &module_candidates(ctx.env))
    };
    rn.report(
        Diagnostic::error(
            "E2002",
            "UNKNOWN MODULE",
            sp,
            format!("I don't know a module named `{q}`"),
        )
        .with_hints(hints),
    );
}

// Diagnostics helpers ----------------------------------------------------------

fn unknown_module_diag(sp: Span, txt: &str, cands: &[String]) -> Diagnostic {
    Diagnostic::error(
        "E2002",
        "UNKNOWN MODULE",
        sp,
        format!("I can't find a module named `{txt}`"),
    )
    .with_hints(suggestion_hints(txt, cands))
}

fn unbound_diag(
    code: &'static str,
    what: &str,
    sp: Span,
    txt: &str,
    cands: &[String],
) -> Diagnostic {
    Diagnostic::error(
        code,
        "UNBOUND NAME",
        sp,
        format!("I don't know a {what} named `{txt}`"),
    )
    .with_hints(suggestion_hints(txt, cands))
}

fn no_member_diag(sp: Span, q: &str, member: &str, cands: &[String]) -> Diagnostic {
    Diagnostic::error(
        "E2003",
        "UNKNOWN MEMBER",
        sp,
        format!("the module `{q}` has nothing named `{member}`"),
    )
    .with_hints(suggestion_hints(member, cands))
}

fn value_candidates(ctx: &Ctx) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // Innermost scope first, matching the Haskell candidate order.
    for s in ctx.scopes.iter().rev() {
        out.extend(s.keys().cloned());
    }
    out.extend(ctx.top_visible.keys().cloned());
    out
}

fn ctor_candidates(ctx: &Ctx) -> Vec<String> {
    let mut out: Vec<String> = ctx.tops.ctors.keys().cloned().collect();
    out.extend(ctx.env.prelude_ctors.keys().cloned());
    out
}

fn module_candidates(env: &RenameEnv) -> Vec<String> {
    env.auto_modules
        .keys()
        .chain(env.user_modules.keys())
        .map(|n| n.0.clone())
        .collect()
}

/// Up to two close names (edit distance <= 2), rendered as hints.
pub fn suggestion_hints(txt: &str, cands: &[String]) -> Vec<String> {
    let txt_chars: Vec<char> = txt.chars().collect();
    let mut scored: Vec<(i32, &String)> = cands
        .iter()
        .filter(|c| c.as_str() != txt)
        .filter_map(|c| {
            let c_chars: Vec<char> = c.chars().collect();
            distance_within(2, &txt_chars, &c_chars).map(|d| (d, c))
        })
        .collect();
    scored.sort_by_key(|(d, _)| *d); // stable: ties keep candidate order
    scored
        .into_iter()
        .take(2)
        .map(|(_, c)| format!("Did you mean `{c}`?"))
        .collect()
}

/// Edit distance if it is <= the budget, else `None`. The budget bounds
/// the branching, so this stays fast on arbitrarily long names.
fn distance_within(budget: i32, a: &[char], b: &[char]) -> Option<i32> {
    if budget < 0 {
        return None;
    }
    match (a, b) {
        ([x, xs @ ..], [y, ys @ ..]) => {
            if x == y {
                distance_within(budget, xs, ys)
            } else {
                [
                    distance_within(budget - 1, xs, b),
                    distance_within(budget - 1, a, ys),
                    distance_within(budget - 1, xs, ys),
                ]
                .into_iter()
                .flatten()
                .min()
                .map(|d| d + 1)
            }
        }
        (xs, []) => (xs.len() as i32 <= budget).then_some(xs.len() as i32),
        ([], ys) => (ys.len() as i32 <= budget).then_some(ys.len() as i32),
    }
}

fn is_upper_text(t: &str) -> bool {
    t.chars().next().is_some_and(|c| c.is_uppercase())
}

#[cfg(test)]
mod tests;
