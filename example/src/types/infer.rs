//! Hindley-Milner inference, Algorithm-W lineage: eager unification with
//! arena-backed metavariables, Rémy levels for let-generalization, rigid
//! skolems for annotations, the Num flavor for overloaded numerics, and a
//! pending-constraint queue for record field access.
//!
//! Per-declaration error isolation: a declaration that produced any
//! diagnostic gets the error type in the environment, so one mistake never
//! cascades (the Elm discipline).

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::*;
use crate::diag::{Diagnostic, dtext};
use crate::maxim_panic;
use crate::names::{ModuleName, Name, NameSort};
use crate::span::Span;
use crate::types::env::*;
use crate::types::ty::*;
use crate::types::unify::{Reason, unify};
use crate::unique::Unique;

// Plumbing ---------------------------------------------------------------------

/// The checker's mutable state: the meta arena, collected diagnostics,
/// the pending field-access queue, and the span-keyed side tables Core
/// lowering and the tooling consume.
pub struct Tc {
    supply: i64,
    pub arena: MetaArena,
    pub diags: Vec<Diagnostic>,
    pending: Vec<PendingField>,
    /// Scrutinee type of every `match`, by the match expression's span —
    /// consumed by exhaustiveness checking and decision-tree compilation.
    pub match_types: BTreeMap<Span, Type>,
    /// The type of every expression, by span — the language server's
    /// hover data.
    pub expr_types: BTreeMap<Span, Type>,
    /// Which failure type each `?` unwraps, by the try expression's
    /// span — consumed by Core lowering (the emitted tag test depends on
    /// it).
    pub try_kinds: BTreeMap<Span, TryKind>,
    /// The conversion function each `::` resolved to, by the cast's
    /// span — consumed by Core lowering. Identity casts have no entry.
    pub cast_fns: BTreeMap<Span, Name>,
    /// How each component tag lowers (saturated view call vs runtime
    /// instance node), by the tag's span — consumed by Core lowering.
    pub instances: BTreeMap<Span, InstLower>,
}

struct PendingField {
    rec_ty: Type,
    label: String,
    span: Span,
    result: Type,
}

#[derive(Clone)]
pub struct Cx {
    pub env: TcEnv,
    pub level: Level,
    pub rigids: BTreeMap<String, TyVar>,
    /// The nearest enclosing fn/lambda return type — where a `?`'s
    /// failure escapes to. None at the top level (a `let` RHS has no
    /// function to return from).
    pub ret: Option<RetCx>,
}

#[derive(Clone)]
pub struct RetCx {
    pub ty: Type,
    /// The enclosing function's name; None for a lambda.
    pub name: Option<String>,
    /// The return annotation's span (else the fn/lambda head) — the
    /// secondary blame site for return conflicts.
    pub span: Span,
}

impl Tc {
    fn new() -> Tc {
        Tc {
            supply: 0,
            arena: MetaArena::new(),
            diags: Vec::new(),
            pending: Vec::new(),
            match_types: BTreeMap::new(),
            expr_types: BTreeMap::new(),
            try_kinds: BTreeMap::new(),
            cast_fns: BTreeMap::new(),
            instances: BTreeMap::new(),
        }
    }

    fn report(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    fn fresh_u(&mut self) -> Unique {
        let u = Unique(self.supply);
        self.supply += 1;
        u
    }

    fn fresh_meta_at(&mut self, level: Level, fl: Flavor) -> Type {
        let u = self.fresh_u();
        Type::Meta(self.arena.alloc(u, fl, level))
    }

    fn fresh_meta(&mut self, cx: &Cx, fl: Flavor) -> Type {
        self.fresh_meta_at(cx.level, fl)
    }

    fn fresh_ty_var(&mut self, txt: &str, fl: Flavor) -> TyVar {
        TyVar {
            unique: self.fresh_u(),
            text: txt.to_string(),
            flavor: fl,
        }
    }

    fn unify_r(&mut self, reason: &Reason, got: &Type, want: &Type) {
        unify(&mut self.arena, &mut self.diags, reason, got, want);
    }

    fn render(&mut self, t: &Type) -> String {
        render_type(&mut self.arena, t)
    }
}

fn bind_val(n: &Name, sch: Scheme, cx: &Cx) -> Cx {
    let mut cx2 = cx.clone();
    cx2.env.values.insert(n.unique, sch);
    cx2
}

fn instantiate(tc: &mut Tc, cx: &Cx, sch: &Scheme) -> Type {
    let mut sub: BTreeMap<Unique, Type> = BTreeMap::new();
    for v in &sch.vars {
        let m = tc.fresh_meta(cx, v.flavor);
        sub.insert(v.unique, m);
    }
    subst_ty_vars(&sub, &sch.ty)
}

fn subst_ty_vars(sub: &BTreeMap<Unique, Type>, ty: &Type) -> Type {
    match ty {
        Type::Var(v) => sub.get(&v.unique).cloned().unwrap_or_else(|| ty.clone()),
        Type::Con(c, args) => Type::Con(
            c.clone(),
            args.iter().map(|a| subst_ty_vars(sub, a)).collect(),
        ),
        Type::Fun(params, ret) => Type::Fun(
            params
                .iter()
                .map(|p| TParam {
                    label: p.label.clone(),
                    ty: subst_ty_vars(sub, &p.ty),
                })
                .collect(),
            Box::new(subst_ty_vars(sub, ret)),
        ),
        t => t.clone(),
    }
}

fn free_ty_vars(ty: &Type) -> Vec<TyVar> {
    fn go(ty: &Type, out: &mut Vec<TyVar>) {
        match ty {
            Type::Var(v) => {
                if !out.contains(v) {
                    out.push(v.clone());
                }
            }
            Type::Con(_, args) => {
                for a in args {
                    go(a, out);
                }
            }
            Type::Fun(params, ret) => {
                for p in params {
                    go(&p.ty, out);
                }
                go(ret, out);
            }
            Type::Meta(_) | Type::Error => {}
        }
    }
    let mut out = Vec::new();
    go(ty, &mut out);
    out
}

/// Quantify metavariables deeper than `cur` (and any free rigids).
fn generalize_at(tc: &mut Tc, cur: Level, ty: &Type) -> Scheme {
    let t = tc.arena.zonk(ty);
    let taken: Vec<String> = free_ty_vars(&t).into_iter().map(|v| v.text).collect();
    let to_gen: Vec<MetaId> = free_metas(&t)
        .into_iter()
        .filter(|m| matches!(tc.arena.cell(*m).content, MetaContent::Unbound(l) if l > cur))
        .collect();
    let (mut n_star, mut n_num) = (0usize, 0usize);
    for m in to_gen {
        let flavor = tc.arena.flavor(m);
        let txt = match flavor {
            Flavor::Star => {
                let t = star_name_avoiding(&taken, n_star);
                n_star += 1;
                t
            }
            Flavor::Num => {
                let t = if n_num == 0 {
                    "number".to_string()
                } else {
                    format!("number{}", n_num + 1)
                };
                n_num += 1;
                t
            }
        };
        let v = tc.fresh_ty_var(&txt, flavor);
        tc.arena.set(m, MetaContent::Bound(Type::Var(v)));
    }
    let t2 = tc.arena.zonk(&t);
    Scheme {
        vars: free_ty_vars(&t2),
        ty: t2,
    }
}

fn star_name_avoiding(taken: &[String], n: usize) -> String {
    ('a'..='z')
        .map(|c| c.to_string())
        .chain((1..).map(|i| format!("t{i}")))
        .filter(|name| !taken.contains(name))
        .nth(n)
        .expect("infinite name supply")
}

// Module checking ----------------------------------------------------------------

enum Assumption {
    Sig(Scheme, BTreeMap<String, TyVar>, Vec<Type>, Type),
    Meta(Type),
}

/// Span-keyed byproducts of a successful check, zonked, for tooling.
#[derive(Clone, Debug, Default)]
pub struct CheckArtifacts {
    /// The scrutinee type of every `match`.
    pub match_types: BTreeMap<Span, Type>,
    /// The type of every expression (the hover map).
    pub expr_types: BTreeMap<Span, Type>,
    /// Which failure type each `?` unwraps (for Core lowering).
    pub try_kinds: BTreeMap<Span, TryKind>,
    /// The conversion function each `::` calls (for Core lowering).
    pub cast_fns: BTreeMap<Span, Name>,
    /// How each component tag lowers (for Core lowering).
    pub instances: BTreeMap<Span, InstLower>,
}

/// Check one module against the environment built from its dependencies'
/// `Iface`s (plus `intrinsics::base_tc_env`). `Err` on any error
/// diagnostic; the `CheckArtifacts` come back either way, so the language
/// server still gets best-effort hover types for the well-typed parts of
/// a file that errors elsewhere.
pub fn check_module(
    env0: &TcEnv,
    m: &Module<Name>,
) -> (Result<Iface, Vec<Diagnostic>>, CheckArtifacts, MetaArena) {
    let mut tc = Tc::new();
    let module_name = &m.name;
    let decls = &m.decls;
    let cx0 = Cx {
        env: env0.clone(),
        level: TOP_LEVEL,
        rigids: BTreeMap::new(),
        ret: None,
    };
    let type_decls: Vec<&TypeDecl<Name>> = decls
        .iter()
        .filter_map(|d| match d {
            Decl::Type(td) => Some(td),
            _ => None,
        })
        .collect();

    // Type declarations: register arity shells first (recursion!), then
    // resolve constructor arguments and fields.
    let mut env_shell = env0.clone();
    for td in &type_decls {
        env_shell.ty_cons.insert(
            td.name.unique,
            TyConInfo {
                con: TyCon {
                    name: td.name.clone(),
                    arity: td.params.len(),
                },
                params: Vec::new(),
                ctors: Vec::new(),
                fields: None,
            },
        );
        env_shell
            .type_names
            .insert(td.name.text.clone(), td.name.clone());
    }
    let cx_shell = Cx {
        env: env_shell.clone(),
        ..cx0.clone()
    };
    let infos: Vec<TyConInfo> = type_decls
        .iter()
        .map(|td| check_type_decl(&mut tc, &cx_shell, td))
        .collect();
    let mut env_types = env_shell;
    for i in &infos {
        env_types.ty_cons.insert(i.con.name.unique, i.clone());
        for c in &i.ctors {
            env_types.ctors.insert(c.name.unique, c.clone());
        }
    }

    // Cast declarations: resolve every signature first (order-independent
    // — a body checked below may cast to a conversion declared later),
    // checking identity, the home rule, and same-module duplicates at the
    // declaration. The resolved table joins the environment for every
    // body check.
    let cx_types = Cx {
        env: env_types.clone(),
        ..cx0.clone()
    };
    let mut casts_here: Vec<((Unique, Unique), &CastDef<Name>, (Name, Scheme))> = Vec::new();
    for d in decls {
        if let Decl::Cast(cd) = d {
            if let Some((key, entry)) = check_cast_sig(&mut tc, &cx_types, module_name, cd) {
                casts_here.push((key, cd, entry));
            }
        }
    }
    let mut cast_table: BTreeMap<(Unique, Unique), (Name, Scheme)> = BTreeMap::new();
    for (key, cd, entry) in &casts_here {
        match cast_table.get(key) {
            None => {
                cast_table.insert(*key, entry.clone());
            }
            Some((prev, _)) => {
                let prev_span = prev.def_span;
                tc.report(
                    Diagnostic::error(
                        "E3030",
                        "DUPLICATE CAST",
                        cd.span,
                        format!(
                            "there is already a cast from `{}` to `{}` in this module",
                            cd.src_head, cd.tgt_head
                        ),
                    )
                    .with_secondary(vec![(prev_span, "the first one is here".to_string())]),
                );
            }
        }
    }
    let mut env_casts = env_types;
    for (k, v) in &cast_table {
        env_casts.casts.insert(*k, v.clone());
    }

    // Assumptions for every value declaration (schemes for fully-annotated
    // ones — polymorphic recursion with a signature; level-0 metas
    // otherwise — monomorphic recursion).
    let cx_casts = Cx {
        env: env_casts.clone(),
        ..cx0.clone()
    };
    let assumptions: Vec<Option<(Name, Assumption)>> = decls
        .iter()
        .map(|d| match d {
            Decl::Type(_) | Decl::Cast(_) | Decl::Component(_) => None,
            Decl::Fn(fnd) => Some((fnd.name.clone(), assume_fn(&mut tc, &cx_casts, fnd))),
            Decl::Let(_, n, ann, _) => {
                Some((n.clone(), assume_let(&mut tc, &cx_casts, ann.as_ref())))
            }
        })
        .collect();
    let mut assumed_env = env_casts;
    for entry in assumptions.iter().flatten() {
        let (n, a) = entry;
        let sch = match a {
            Assumption::Sig(sch, _, _, _) => sch.clone(),
            Assumption::Meta(m) => mono_scheme(m.clone()),
        };
        assumed_env.values.insert(n.unique, sch);
    }

    // Component manifests: derive each signature from the wired
    // functions' (annotated) schemes so tags check in any body below,
    // declaration order notwithstanding.
    let assumed_schemes: BTreeMap<Unique, Scheme> = assumptions
        .iter()
        .flatten()
        .filter_map(|(n, a)| match a {
            Assumption::Sig(sch, _, _, _) => Some((n.unique, sch.clone())),
            Assumption::Meta(_) => None,
        })
        .collect();
    let cx_assumed = Cx {
        env: assumed_env.clone(),
        ..cx0.clone()
    };
    let comp_sigs: Vec<CompSig> = decls
        .iter()
        .filter_map(|d| match d {
            Decl::Component(cd) => check_component(&mut tc, &cx_assumed, &assumed_schemes, cd),
            _ => None,
        })
        .collect();
    let comp_table: BTreeMap<Unique, CompSig> = comp_sigs
        .iter()
        .map(|s| (s.name.unique, s.clone()))
        .collect();
    let mut env_comps = assumed_env;
    for (k, v) in &comp_table {
        env_comps.components.insert(*k, v.clone());
    }

    // Check bodies in order, replacing each assumption with the real
    // scheme as it lands (and the error scheme when a declaration failed).
    let mut cx = Cx {
        env: env_comps,
        ..cx0.clone()
    };
    let mut finals: Vec<(Name, Scheme)> = Vec::new();
    for (d, ma) in decls.iter().zip(&assumptions) {
        match (d, ma) {
            (Decl::Fn(fnd), Some((_, a))) => {
                let (n, sch) = isolated(&mut tc, &cx, fnd.name.clone(), |tc| {
                    check_fn(tc, &cx, fnd, a)
                });
                cx = bind_val(&n, sch.clone(), &cx);
                finals.push((n, sch));
            }
            (Decl::Let(_, n, ann, e), Some((_, a))) => {
                let (n2, sch) = isolated(&mut tc, &cx, n.clone(), |tc| {
                    check_top_let(tc, &cx, n, ann.as_ref(), e, a)
                });
                cx = bind_val(&n2, sch.clone(), &cx);
                finals.push((n2, sch));
            }
            _ => {}
        }
    }

    // Cast bodies check last, against the final environment: their
    // signatures are already in the table (they need no inference), and a
    // cast body may call any function — or any cast — in the module.
    for (_, cd, (_, sch)) in &casts_here {
        check_cast_body(&mut tc, &cx, cd, sch);
    }

    // Artifacts are built unconditionally (zonked), so an errored module
    // still yields the partial hover/match types it managed to infer.
    let match_types: BTreeMap<Span, Type> = tc
        .match_types
        .clone()
        .into_iter()
        .map(|(sp, t)| {
            let z = tc.arena.zonk(&t);
            (sp, z)
        })
        .collect();
    let expr_types: BTreeMap<Span, Type> = tc
        .expr_types
        .clone()
        .into_iter()
        .map(|(sp, t)| {
            let z = tc.arena.zonk(&t);
            (sp, z)
        })
        .collect();
    let artifacts = CheckArtifacts {
        match_types,
        expr_types,
        try_kinds: tc.try_kinds.clone(),
        cast_fns: tc.cast_fns.clone(),
        instances: tc.instances.clone(),
    };
    if tc.diags.is_empty() {
        let if_values: BTreeMap<Unique, Scheme> = finals
            .iter()
            .map(|(n, s)| (n.unique, tc.arena.zonk_scheme(s)))
            .collect();
        (
            Ok(Iface {
                values: if_values,
                ctors: infos
                    .iter()
                    .flat_map(|i| i.ctors.iter())
                    .map(|c| (c.name.unique, c.clone()))
                    .collect(),
                ty_cons: infos
                    .iter()
                    .map(|i| (i.con.name.unique, i.clone()))
                    .collect(),
                casts: cast_table,
                components: comp_table,
            }),
            artifacts,
            tc.arena,
        )
    } else {
        (Err(std::mem::take(&mut tc.diags)), artifacts, tc.arena)
    }
}

/// Run a declaration check; if it reported anything, its public type
/// becomes the error type (cascade suppression).
fn isolated(tc: &mut Tc, cx: &Cx, n: Name, act: impl FnOnce(&mut Tc) -> Scheme) -> (Name, Scheme) {
    let before = tc.diags.len();
    let sch = act(tc);
    flush_pending(tc, cx);
    let after = tc.diags.len();
    (
        n,
        if after > before {
            mono_scheme(Type::Error)
        } else {
            sch
        },
    )
}

// Type declarations --------------------------------------------------------------

fn check_type_decl(tc: &mut Tc, cx: &Cx, td: &TypeDecl<Name>) -> TyConInfo {
    let mut params: Vec<TyVar> = Vec::with_capacity(td.params.len());
    for (sp, txt) in &td.params {
        if txt == "number" {
            tc.report(
                Diagnostic::error(
                    "E3015",
                    "RESERVED TYPE VARIABLE",
                    *sp,
                    "`number` can't be a type parameter",
                )
                .with_hints(vec![
                    "`number` is reserved: it is the type variable that ranges over Int and Float."
                        .to_string(),
                ]),
            );
        }
        params.push(tc.fresh_ty_var(txt, Flavor::Star));
    }
    let scope: BTreeMap<String, TyVar> = td
        .params
        .iter()
        .map(|(_, txt)| txt.clone())
        .zip(params.iter().cloned())
        .collect();
    let tycon = TyCon {
        name: td.name.clone(),
        arity: params.len(),
    };
    let cx2 = Cx {
        rigids: scope,
        ..cx.clone()
    };
    match &td.rhs {
        TypeRhs::Variants(vs) => {
            let ctors: Vec<CtorInfo> = vs
                .iter()
                .map(|v| CtorInfo {
                    name: v.name.clone(),
                    ty_con: tycon.clone(),
                    params: params.clone(),
                    args: v.args.iter().map(|a| resolve_ann(tc, &cx2, a)).collect(),
                    fields: None,
                })
                .collect();
            TyConInfo {
                con: tycon,
                params,
                ctors,
                fields: None,
            }
        }
        TypeRhs::Record(ctor_name, fields) => {
            for (i, f) in fields.iter().enumerate() {
                if fields[..i].iter().any(|g| g.name == f.name) {
                    tc.report(Diagnostic::error(
                        "E3017",
                        "DUPLICATE FIELD",
                        f.span,
                        format!("the field `{}` appears twice", f.name),
                    ));
                }
            }
            let ftys: Vec<(String, Type)> = fields
                .iter()
                .map(|f| (f.name.clone(), resolve_ann(tc, &cx2, &f.ty)))
                .collect();
            let ci = CtorInfo {
                name: ctor_name.clone(),
                ty_con: tycon.clone(),
                params: params.clone(),
                args: ftys.iter().map(|(_, t)| t.clone()).collect(),
                fields: Some(ftys.iter().map(|(n, _)| n.clone()).collect()),
            };
            TyConInfo {
                con: tycon,
                params,
                ctors: vec![ci],
                fields: Some(ftys),
            }
        }
    }
}

// Annotations --------------------------------------------------------------------

fn resolve_ann(tc: &mut Tc, cx: &Cx, ann: &TypeAnn) -> Type {
    match ann {
        TypeAnn::Var(sp, txt) => match cx.rigids.get(txt) {
            Some(v) => Type::Var(v.clone()),
            None => {
                tc.report(
                    Diagnostic::error(
                        "E3011",
                        "UNKNOWN TYPE VARIABLE",
                        *sp,
                        format!("I don't know the type variable `{txt}`"),
                    )
                    .with_hints(vec![
                        "Type variables must be introduced by the enclosing signature or type declaration."
                            .to_string(),
                    ]),
                );
                Type::Error
            }
        },
        TypeAnn::Name(sp, txt, args) => match cx.env.type_names.get(txt) {
            None => {
                let cands: Vec<String> = cx.env.type_names.keys().cloned().collect();
                tc.report(
                    Diagnostic::error(
                        "E3001",
                        "UNKNOWN TYPE",
                        *sp,
                        format!("I don't know a type named `{txt}`"),
                    )
                    .with_hints(did_you_mean(txt, &cands)),
                );
                Type::Error
            }
            Some(n) => match cx.env.ty_cons.get(&n.unique).cloned() {
                None => Type::Error,
                Some(ti) => {
                    if args.len() != ti.con.arity {
                        tc.report(Diagnostic::error(
                            "E3013",
                            "WRONG NUMBER OF TYPE ARGUMENTS",
                            *sp,
                            format!(
                                "`{txt}` expects {}, but I see {}",
                                count_text(ti.con.arity, "type argument"),
                                args.len()
                            ),
                        ));
                        Type::Error
                    } else {
                        Type::Con(
                            ti.con.clone(),
                            args.iter().map(|a| resolve_ann(tc, cx, a)).collect(),
                        )
                    }
                }
            },
        },
        TypeAnn::Fun(_, args, ret) => Type::Fun(
            args.iter()
                .map(|(ml, a)| TParam {
                    label: ml.clone(),
                    ty: resolve_ann(tc, cx, a),
                })
                .collect(),
            Box::new(resolve_ann(tc, cx, ret)),
        ),
    }
}

/// Free type-variable texts of a signature, first-appearance order.
fn sig_ty_var_texts(anns: &[&TypeAnn]) -> Vec<String> {
    fn go(ann: &TypeAnn, out: &mut Vec<String>) {
        match ann {
            TypeAnn::Var(_, t) => {
                if !out.contains(t) {
                    out.push(t.clone());
                }
            }
            TypeAnn::Name(_, _, args) => {
                for a in args {
                    go(a, out);
                }
            }
            TypeAnn::Fun(_, args, ret) => {
                for (_, a) in args {
                    go(a, out);
                }
                go(ret, out);
            }
        }
    }
    let mut out = Vec::new();
    for a in anns {
        go(a, &mut out);
    }
    out
}

fn flavor_of_text(t: &str) -> Flavor {
    if t == "number" {
        Flavor::Num
    } else {
        Flavor::Star
    }
}

fn rigid_scope_for(tc: &mut Tc, anns: &[&TypeAnn]) -> BTreeMap<String, TyVar> {
    sig_ty_var_texts(anns)
        .into_iter()
        .map(|t| {
            let v = tc.fresh_ty_var(&t, flavor_of_text(&t));
            (t, v)
        })
        .collect()
}

// Value declarations -------------------------------------------------------------

/// Parameter names become the call labels of the function's type (`_`
/// params stay unlabeled).
fn label_params(params: &[FnParam<Name>], tys: &[Type]) -> Vec<TParam> {
    params
        .iter()
        .zip(tys)
        .map(|(p, t)| TParam {
            label: p.name.as_ref().map(|n| n.text.clone()),
            ty: t.clone(),
        })
        .collect()
}

fn assume_fn(tc: &mut Tc, cx: &Cx, fnd: &FnDef<Name>) -> Assumption {
    if fnd.params.iter().all(|p| p.ann.is_some()) {
        if let Some(ret) = &fnd.ret {
            let mut anns: Vec<&TypeAnn> =
                fnd.params.iter().filter_map(|p| p.ann.as_ref()).collect();
            anns.push(ret);
            let scope = rigid_scope_for(tc, &anns);
            let cx2 = Cx {
                rigids: scope.clone(),
                ..cx.clone()
            };
            let ptys: Vec<Type> = fnd
                .params
                .iter()
                .filter_map(|p| p.ann.as_ref())
                .map(|a| resolve_ann(tc, &cx2, a))
                .collect();
            let rty = resolve_ann(tc, &cx2, ret);
            let sch = Scheme {
                vars: scope.values().cloned().collect(),
                ty: Type::Fun(label_params(&fnd.params, &ptys), Box::new(rty.clone())),
            };
            return Assumption::Sig(sch, scope, ptys, rty);
        }
    }
    Assumption::Meta(tc.fresh_meta(cx, Flavor::Star))
}

fn assume_let(tc: &mut Tc, cx: &Cx, ann: Option<&TypeAnn>) -> Assumption {
    match ann {
        Some(a) => {
            let scope = rigid_scope_for(tc, &[a]);
            let cx2 = Cx {
                rigids: scope.clone(),
                ..cx.clone()
            };
            let ty = resolve_ann(tc, &cx2, a);
            Assumption::Sig(
                Scheme {
                    vars: scope.values().cloned().collect(),
                    ty: ty.clone(),
                },
                scope,
                Vec::new(),
                ty,
            )
        }
        None => Assumption::Meta(tc.fresh_meta(cx, Flavor::Star)),
    }
}

fn check_fn(tc: &mut Tc, cx: &Cx, fnd: &FnDef<Name>, assumption: &Assumption) -> Scheme {
    let ret_cx = |rty: &Type| RetCx {
        ty: rty.clone(),
        name: Some(fnd.name.text.clone()),
        span: fnd.ret.as_ref().map_or(fnd.span, |r| r.span()),
    };
    let bind_params = |params_tys: &[Type], c: &Cx| -> Cx {
        let mut acc = c.clone();
        for (p, t) in fnd.params.iter().zip(params_tys) {
            if let Some(n) = &p.name {
                acc = bind_val(n, mono_scheme(t.clone()), &acc);
            }
        }
        acc
    };
    match assumption {
        Assumption::Sig(sch, scope, ptys, rty) => {
            let mut cx_body = bind_params(ptys, cx);
            cx_body.rigids = scope.clone();
            cx_body.level = cx.level.bump();
            cx_body.ret = Some(ret_cx(rty));
            let body_ty = infer_expr(tc, &cx_body, &fnd.body);
            tc.unify_r(
                &Reason::RetAnn(fnd.name.text.clone(), fnd.body.span()),
                &body_ty,
                rty,
            );
            sch.clone()
        }
        Assumption::Meta(m) => {
            let present_anns: Vec<&TypeAnn> = fnd
                .params
                .iter()
                .filter_map(|p| p.ann.as_ref())
                .chain(fnd.ret.as_ref())
                .collect();
            let scope = rigid_scope_for(tc, &present_anns);
            let cx_sig = Cx {
                rigids: scope,
                ..cx.clone()
            };
            let ptys: Vec<Type> = fnd
                .params
                .iter()
                .map(|p| match &p.ann {
                    Some(a) => resolve_ann(tc, &cx_sig, a),
                    None => tc.fresh_meta_at(cx.level.bump(), Flavor::Star),
                })
                .collect();
            // The return type exists before the body is inferred, so a `?`
            // inside can constrain it (early returns flow into it).
            let (ret_ty, ret_reason) = match &fnd.ret {
                Some(r) => {
                    let rty = resolve_ann(tc, &cx_sig, r);
                    (rty, Reason::RetAnn(fnd.name.text.clone(), fnd.body.span()))
                }
                None => {
                    let rm = tc.fresh_meta_at(cx.level.bump(), Flavor::Star);
                    (rm, Reason::LamRet(fnd.body.span()))
                }
            };
            let mut cx_body = bind_params(&ptys, &cx_sig);
            cx_body.level = cx.level.bump();
            cx_body.ret = Some(ret_cx(&ret_ty));
            let body_ty = infer_expr(tc, &cx_body, &fnd.body);
            tc.unify_r(&ret_reason, &body_ty, &ret_ty);
            let fn_ty = Type::Fun(label_params(&fnd.params, &ptys), Box::new(ret_ty));
            tc.unify_r(&Reason::RetAnn(fnd.name.text.clone(), fnd.span), &fn_ty, m);
            generalize_at(tc, TOP_LEVEL, &fn_ty)
        }
    }
}

fn check_top_let(
    tc: &mut Tc,
    cx: &Cx,
    n: &Name,
    _ann: Option<&TypeAnn>,
    e: &Expr<Name>,
    assumption: &Assumption,
) -> Scheme {
    match assumption {
        Assumption::Sig(sch, scope, _, ty) => {
            let cx2 = Cx {
                rigids: scope.clone(),
                level: cx.level.bump(),
                ..cx.clone()
            };
            let rhs_ty = infer_expr(tc, &cx2, e);
            tc.unify_r(&Reason::LetAnn(e.span()), &rhs_ty, ty);
            sch.clone()
        }
        Assumption::Meta(m) => {
            let cx2 = Cx {
                level: cx.level.bump(),
                ..cx.clone()
            };
            let rhs_ty = infer_expr(tc, &cx2, e);
            tc.unify_r(&Reason::RetAnn(n.text.clone(), e.span()), &rhs_ty, m);
            generalize_at(tc, TOP_LEVEL, &rhs_ty)
        }
    }
}

// Expressions --------------------------------------------------------------------

pub fn infer_expr(tc: &mut Tc, cx: &Cx, e0: &Expr<Name>) -> Type {
    let t = infer_expr_inner(tc, cx, e0);
    tc.expr_types.insert(e0.span(), t.clone());
    t
}

fn infer_expr_inner(tc: &mut Tc, cx: &Cx, expr: &Expr<Name>) -> Type {
    match expr {
        Expr::Var(_, n) => {
            if n.sort == NameSort::Error {
                Type::Error
            } else {
                match cx.env.values.get(&n.unique).cloned() {
                    Some(sch) => instantiate(tc, cx, &sch),
                    None => Type::Error,
                }
            }
        }
        Expr::Con(sp, n) => con_ref_type(tc, cx, *sp, n),
        Expr::Lit(_, l) => lit_type(tc, cx, l),
        Expr::Dot(_, _, _, _) => Type::Error,
        Expr::Field(_, e, f_sp, fld) => {
            let rec_ty = infer_expr(tc, cx, e);
            let res = tc.fresh_meta(cx, Flavor::Star);
            tc.pending.push(PendingField {
                rec_ty,
                label: fld.clone(),
                span: *f_sp,
                result: res.clone(),
            });
            try_solve_pending(tc, cx);
            res
        }
        Expr::Call(sp, f, args) => {
            let pre: Vec<PreArg> = args.iter().map(PreArg::Arg).collect();
            infer_call(tc, cx, *sp, f, &pre)
        }
        Expr::Pipe(sp, style, l, r) => {
            let l_ty = infer_expr(tc, cx, l);
            let pre = PreArg::Pre(l_ty, l.span());
            match (style, &**r) {
                (PipeStyle::Into, Expr::Call(_, f, args)) => {
                    let mut pres = vec![pre];
                    pres.extend(args.iter().map(PreArg::Arg));
                    infer_call(tc, cx, *sp, f, &pres)
                }
                _ => infer_call(tc, cx, *sp, r, &[pre]),
            }
        }
        Expr::Bin(_, op, op_sp, l, r) => infer_bin(tc, cx, *op, *op_sp, l, r),
        Expr::Unary(_, op, e) => {
            let t = infer_expr(tc, cx, e);
            match op {
                UnOp::Neg => {
                    let m = tc.fresh_meta(cx, Flavor::Num);
                    tc.unify_r(&Reason::Unary(UnOp::Neg, e.span()), &t, &m);
                    m
                }
                UnOp::Not => {
                    tc.unify_r(&Reason::Unary(UnOp::Not, e.span()), &t, &t_bool());
                    t_bool()
                }
            }
        }
        Expr::If(_, c, t, e) => {
            let ct = infer_expr(tc, cx, c);
            tc.unify_r(&Reason::IfCond(c.span()), &ct, &t_bool());
            let tt = infer_expr(tc, cx, t);
            let et = infer_expr(tc, cx, e);
            tc.unify_r(&Reason::IfBranches(e.span()), &et, &tt);
            strip_disagreeing_labels(tc, &tt, &[et])
        }
        Expr::Match(sp, scrut, arms) => {
            let s_ty = infer_expr(tc, cx, scrut);
            tc.match_types.insert(*sp, s_ty.clone());
            let res_ty = tc.fresh_meta(cx, Flavor::Star);
            let mut arm_tys = Vec::with_capacity(arms.len());
            for (i, arm) in arms.iter().enumerate() {
                let binds = check_pat(tc, cx, &arm.pat, &s_ty);
                let mut cx2 = cx.clone();
                for (n, t) in binds {
                    cx2 = bind_val(&n, mono_scheme(t), &cx2);
                }
                if let Some(g) = &arm.guard {
                    let g_ty = infer_expr(tc, &cx2, g);
                    tc.unify_r(&Reason::IfCond(g.span()), &g_ty, &t_bool());
                }
                let b_ty = infer_expr(tc, &cx2, &arm.body);
                tc.unify_r(&Reason::MatchArm(i + 1, arm.body.span()), &b_ty, &res_ty);
                arm_tys.push(b_ty);
            }
            strip_disagreeing_labels(tc, &res_ty, &arm_tys)
        }
        Expr::Cast(sp, e, ty_sp, ty_txt) => infer_cast(tc, cx, *sp, e, *ty_sp, ty_txt),
        Expr::Lam(sp, params, ret, body) => {
            // Annotations on lambda params resolve against the ENCLOSING
            // rigid scope: a lambda may use its function's signature
            // variables or concrete types, but introduces no polymorphism
            // of its own (an unknown variable is E3011).
            let ptys: Vec<Type> = params
                .iter()
                .map(|p| match &p.ann {
                    Some(a) => resolve_ann(tc, cx, a),
                    None => tc.fresh_meta(cx, Flavor::Star),
                })
                .collect();
            // The lambda's return type is its own (annotated or a fresh
            // meta), so a `?` in the body targets THIS lambda, not the
            // enclosing function.
            let ret_m = match ret {
                Some(r) => resolve_ann(tc, cx, r),
                None => tc.fresh_meta(cx, Flavor::Star),
            };
            let mut cx2 = cx.clone();
            for (p, t) in params.iter().zip(&ptys) {
                if let Some(n) = &p.name {
                    cx2 = bind_val(n, mono_scheme(t.clone()), &cx2);
                }
            }
            cx2.ret = Some(RetCx {
                ty: ret_m.clone(),
                name: None,
                span: *sp,
            });
            let b_ty = infer_expr(tc, &cx2, body);
            tc.unify_r(&Reason::LamRet(body.span()), &b_ty, &ret_m);
            Type::Fun(label_params(params, &ptys), Box::new(ret_m))
        }
        Expr::List(_, elems, spread) => {
            let el = tc.fresh_meta(cx, Flavor::Star);
            for (i, e) in elems.iter().enumerate() {
                let t = infer_expr(tc, cx, e);
                tc.unify_r(&Reason::ListElem(i + 1, e.span()), &t, &el);
            }
            if let Some(e) = spread {
                let t = infer_expr(tc, cx, e);
                tc.unify_r(&Reason::ListSpread(e.span()), &t, &t_list(el.clone()));
            }
            t_list(el)
        }
        Expr::Block(_, stmts, result) => infer_block(tc, cx, stmts, result),
        Expr::Update(sp, ctor, _ctor_sp, base, fields) => {
            infer_update(tc, cx, *sp, ctor, base, fields)
        }
        Expr::Try(sp, q_sp, inner) => {
            let inner_ty = infer_expr(tc, cx, inner);
            infer_try(tc, cx, *sp, *q_sp, inner.span(), &inner_ty)
        }
        Expr::Interp(_, parts) => {
            for p in parts {
                if let InterpPart::Expr(e) = p {
                    let t = infer_expr(tc, cx, e);
                    tc.unify_r(&Reason::Interp(e.span()), &t, &t_string());
                }
            }
            t_string()
        }
        Expr::Jsx(_, el) => infer_jsx(tc, cx, el),
        Expr::Instance(_, _, _, _) => {
            maxim_panic!("infer_expr: EInstance is synthesized by Core lowering")
        }
    }
}

// JSX ---------------------------------------------------------------------------

/// A JSX element builds the stdlib `Html<msg>` — pinned by the driver,
/// so a local `type Html` shadow cannot change what elements construct
/// (the emitted code calls the stdlib's `Html.node`).
fn infer_jsx(tc: &mut Tc, cx: &Cx, el: &JsxElem<Name>) -> Type {
    match cx.env.jsx_html.clone() {
        None => {
            tc.report(
                Diagnostic::error(
                    "E3033",
                    "JSX NEEDS HTML",
                    el.span,
                    "the stdlib `Html` module isn't available, so JSX has nothing to build",
                )
                .with_hints(vec![
                    "JSX builds `Html<msg>` values; the stdlib's `Html` module provides the type."
                        .to_string(),
                ]),
            );
            drain_jsx(tc, cx, el);
            Type::Error
        }
        Some(html_con) => infer_jsx_elem(tc, cx, &html_con, el),
    }
}

/// Check every expression inside an element that already failed, so
/// nested errors still surface (the `drain_all` discipline for calls).
fn drain_jsx(tc: &mut Tc, cx: &Cx, el: &JsxElem<Name>) {
    for a in &el.attrs {
        match a {
            JsxAttr::Attr(_, _, v) => {
                if let Some(e) = v {
                    infer_expr(tc, cx, e);
                }
            }
            JsxAttr::On(_, _, e) => {
                infer_expr(tc, cx, e);
            }
        }
    }
    for c in &el.children {
        match c {
            JsxChild::Text(_, _) => {}
            JsxChild::Elem(el2) => drain_jsx(tc, cx, el2),
            JsxChild::Splice(_, e) | JsxChild::Spread(_, e) | JsxChild::Hole(_, e) => {
                infer_expr(tc, cx, e);
            }
        }
    }
}

fn infer_jsx_elem(tc: &mut Tc, cx: &Cx, html_con: &TyCon, el: &JsxElem<Name>) -> Type {
    let t = match &el.tag {
        JsxTag::Intrinsic(_, _) => {
            // Handlers pin the message type; a handler-free element stays
            // polymorphic in it.
            let msg = tc.fresh_meta(cx, Flavor::Star);
            let this = Type::Con(html_con.clone(), vec![msg.clone()]);
            for a in &el.attrs {
                match a {
                    JsxAttr::Attr(_, name, v) => {
                        if let Some(e) = v {
                            let t2 = infer_expr(tc, cx, e);
                            tc.unify_r(&Reason::JsxAttr(name.clone(), e.span()), &t2, &t_string());
                        }
                    }
                    JsxAttr::On(_, ev, e) => {
                        let t2 = infer_expr(tc, cx, e);
                        // The payload table: an `input` handler takes the
                        // target's value, a `change` handler the checkbox
                        // state; every other event's handler is a plain
                        // message.
                        let want = match ev.as_str() {
                            "input" => t_fun(vec![t_string()], msg.clone()),
                            "change" => t_fun(vec![t_bool()], msg.clone()),
                            _ => msg.clone(),
                        };
                        tc.unify_r(&Reason::JsxHandler(ev.clone(), e.span()), &t2, &want);
                    }
                }
            }
            for c in &el.children {
                infer_jsx_child(tc, cx, html_con, &this, c);
            }
            this
        }
        JsxTag::Component(tag_sp, view_name) => {
            infer_jsx_component(tc, cx, html_con, el, *tag_sp, view_name)
        }
    };
    // Hover works on nested elements too.
    tc.expr_types.insert(el.span, t.clone());
    t
}

fn infer_jsx_child(tc: &mut Tc, cx: &Cx, html_con: &TyCon, this: &Type, c: &JsxChild<Name>) {
    match c {
        JsxChild::Text(_, _) => {}
        JsxChild::Elem(child) => {
            let t = infer_jsx_elem(tc, cx, html_con, child);
            tc.unify_r(&Reason::JsxChild(child.span), &t, this);
        }
        JsxChild::Splice(_, e) => {
            let t = infer_expr(tc, cx, e);
            tc.unify_r(&Reason::JsxSplice(e.span()), &t, this);
        }
        JsxChild::Spread(_, e) => {
            let t = infer_expr(tc, cx, e);
            tc.unify_r(&Reason::JsxSpread(e.span()), &t, &t_list(this.clone()));
        }
        JsxChild::Hole(_, e) => {
            let t = infer_expr(tc, cx, e);
            tc.unify_r(&Reason::Interp(e.span()), &t, &t_string());
        }
    }
}

/// A capitalized tag resolves to a `component` declaration. A STATELESS
/// one is a SATURATED labeled call of the wired view: attributes are the
/// labels, and `children:` is passed exactly when the element has a
/// closing tag (missing attribute E3034, unknown E3035). A STATEFUL one
/// builds a runtime instance node: attributes are Props fields (`Option`
/// fields omittable), `on:` names the component's out events.
fn infer_jsx_component(
    tc: &mut Tc,
    cx: &Cx,
    html_con: &TyCon,
    el: &JsxElem<Name>,
    tag_sp: Span,
    comp_n: &Name,
) -> Type {
    if comp_n.sort == NameSort::Error {
        drain_jsx(tc, cx, el);
        return Type::Error;
    }
    match cx.env.components.get(&comp_n.unique).cloned() {
        // The manifest failed its own check (E3036, already reported).
        None => {
            drain_jsx(tc, cx, el);
            Type::Error
        }
        Some(sig) => {
            if sig.stateful {
                infer_instance_tag(tc, cx, html_con, el, tag_sp, &sig)
            } else {
                tc.instances
                    .insert(tag_sp, InstLower::Call(sig.view.clone()));
                infer_stateless_tag(tc, cx, html_con, el, tag_sp, &sig)
            }
        }
    }
}

/// The stateless path — today's component-call semantics, aimed at the
/// manifest's wired view.
fn infer_stateless_tag(
    tc: &mut Tc,
    cx: &Cx,
    html_con: &TyCon,
    el: &JsxElem<Name>,
    tag_sp: Span,
    sig: &CompSig,
) -> Type {
    let view_name = &sig.view;
    let f_ty0 = match cx.env.values.get(&view_name.unique).cloned() {
        Some(sch) => instantiate(tc, cx, &sch),
        None => Type::Error,
    };
    let f_ty = tc.arena.zonk(&f_ty0);
    let desc = format!("component `{}`", sig.name.text);
    // `on:` names an out event, and a stateless component has none.
    for a in &el.attrs {
        if let JsxAttr::On(asp, ev, h) = a {
            infer_expr(tc, cx, h);
            tc.report(
                Diagnostic::error(
                    "E1115",
                    "UNKNOWN OUT EVENT",
                    *asp,
                    format!("{desc} is stateless — it has no out event `{ev}`"),
                )
                .with_hints(vec![format!(
                    "Out events come from a stateful component's `Out` type; {desc} wires only a view. Pass the handler as an ordinary attribute instead."
                )]),
            );
        }
    }
    match f_ty {
        Type::Error => {
            drain_jsx(tc, cx, el);
            Type::Error
        }
        Type::Fun(ps, ret) => {
            // Core lowering reads the callee's type at the TAG span and
            // reuses the ordinary labeled-call machinery.
            tc.expr_types
                .insert(tag_sp, Type::Fun(ps.clone(), ret.clone()));
            let given: Vec<(&String, &Expr<Name>, Span)> = el
                .attrs
                .iter()
                .filter_map(|a| match a {
                    JsxAttr::Attr(_, name, Some(e)) => Some((name, e, a.span())),
                    _ => None,
                })
                .collect();
            let by_label: BTreeMap<&String, (usize, &Type)> = ps
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.label.as_ref().map(|l| (l, (i + 1, &p.ty))))
                .collect();
            let plabels: Vec<&String> = ps.iter().filter_map(|p| p.label.as_ref()).collect();
            let child_lbl = "children".to_string();
            let mut filled: Vec<&String> = given.iter().map(|(n, _, _)| *n).collect();
            if el.has_close {
                filled.push(&child_lbl);
            }
            let missing: Vec<&String> = plabels
                .iter()
                .filter(|l| !filled.contains(l))
                .copied()
                .collect();
            let unnamed_count = ps.iter().filter(|p| p.label.is_none()).count();
            let takes_children = plabels.iter().any(|l| **l == child_lbl);
            for (name, e, asp) in &given {
                let t = infer_expr(tc, cx, e);
                match by_label.get(name) {
                    Some((i, want)) => {
                        tc.unify_r(&Reason::CallArg(*i, desc.clone(), e.span()), &t, want);
                    }
                    None => {
                        let cands: Vec<String> = plabels.iter().map(|l| (*l).clone()).collect();
                        tc.report(
                            Diagnostic::error(
                                "E3035",
                                "UNKNOWN ATTRIBUTE",
                                *asp,
                                format!("{desc} has no parameter named `{name}`"),
                            )
                            .with_hints(did_you_mean(name, &cands)),
                        );
                    }
                }
            }
            let children_ok = if el.has_close {
                let msg = tc.fresh_meta(cx, Flavor::Star);
                let this = Type::Con(html_con.clone(), vec![msg]);
                for c in &el.children {
                    infer_jsx_child(tc, cx, html_con, &this, c);
                }
                match by_label.get(&child_lbl) {
                    Some((i, want)) => {
                        tc.unify_r(
                            &Reason::CallArg(*i, desc.clone(), el.span),
                            &t_list(this),
                            want,
                        );
                        true
                    }
                    None => {
                        tc.report(
                            Diagnostic::error(
                                "E3035",
                                "UNKNOWN ATTRIBUTE",
                                el.span,
                                format!(
                                    "{desc} has no `children` parameter, but this element has a closing tag"
                                ),
                            )
                            .with_hints(vec![format!("Self-close it — {desc} takes no children.")]),
                        );
                        false
                    }
                }
            } else {
                true
            };
            for l in &missing {
                tc.report(
                    Diagnostic::error(
                        "E3034",
                        "MISSING ATTRIBUTE",
                        tag_sp,
                        format!("{desc} needs `{l}`"),
                    )
                    .with_hints(vec![if **l == child_lbl {
                        "Children fill it: write `<…></…>` (an empty pair passes `children: []`)."
                            .to_string()
                    } else {
                        format!("Add `{l}={{…}}` to the tag.")
                    }]),
                );
            }
            if unnamed_count > 0 {
                tc.report(Diagnostic::error(
                    "E3034",
                    "MISSING ATTRIBUTE",
                    tag_sp,
                    format!(
                        "{desc} has an unnamed parameter; a component tag passes every argument by name"
                    ),
                ));
            }
            let saturated = missing.is_empty()
                && unnamed_count == 0
                && children_ok
                && given.iter().all(|(n, _, _)| plabels.contains(n))
                && (takes_children || !el.has_close);
            if saturated { *ret } else { Type::Error }
        }
        other => {
            drain_jsx(tc, cx, el);
            let other_r = tc.render(&other);
            tc.report(Diagnostic::error(
                "E3006",
                "NOT A FUNCTION",
                tag_sp,
                format!("{desc} is `{other_r}`, so it cannot be a component"),
            ));
            Type::Error
        }
    }
}

/// The stateful path: the tag builds a runtime instance node.
fn infer_instance_tag(
    tc: &mut Tc,
    cx: &Cx,
    html_con: &TyCon,
    el: &JsxElem<Name>,
    tag_sp: Span,
    sig: &CompSig,
) -> Type {
    let msg = tc.fresh_meta(cx, Flavor::Star);
    let this = Type::Con(html_con.clone(), vec![msg.clone()]);
    let desc = format!("component `{}`", sig.name.text);
    let props_by_lbl: BTreeMap<&String, &Type> = sig.props.iter().map(|(l, t, _)| (l, t)).collect();
    let outs_by_name: BTreeMap<&String, &Vec<Type>> =
        sig.outs.iter().map(|(n, ps)| (n, ps)).collect();
    for a in &el.attrs {
        match a {
            JsxAttr::Attr(asp, name, mv) => match props_by_lbl.get(name) {
                Some(want) => {
                    if let Some(e) = mv {
                        let t = infer_expr(tc, cx, e);
                        tc.unify_r(&Reason::JsxAttr(name.clone(), e.span()), &t, want);
                    }
                }
                None => {
                    if let Some(e) = mv {
                        infer_expr(tc, cx, e);
                    }
                    let cands: Vec<String> = sig.props.iter().map(|(l, _, _)| l.clone()).collect();
                    tc.report(
                        Diagnostic::error(
                            "E3035",
                            "UNKNOWN ATTRIBUTE",
                            *asp,
                            format!("{desc} has no prop named `{name}`"),
                        )
                        .with_hints(did_you_mean(name, &cands)),
                    );
                }
            },
            JsxAttr::On(asp, ev, h) => match outs_by_name.get(ev) {
                Some(payloads) => {
                    let t = infer_expr(tc, cx, h);
                    let want = if payloads.is_empty() {
                        msg.clone()
                    } else {
                        t_fun((*payloads).clone(), msg.clone())
                    };
                    tc.unify_r(&Reason::JsxHandler(ev.clone(), h.span()), &t, &want);
                }
                None => {
                    infer_expr(tc, cx, h);
                    let cands: Vec<String> = sig.outs.iter().map(|(n, _)| n.clone()).collect();
                    let mut hints = did_you_mean(ev, &cands);
                    hints.push(
                        "A component's `on:` events are its `Out` constructors, lowercased."
                            .to_string(),
                    );
                    tc.report(
                        Diagnostic::error(
                            "E1115",
                            "UNKNOWN OUT EVENT",
                            *asp,
                            format!("{desc} has no out event `{ev}`"),
                        )
                        .with_hints(hints),
                    );
                }
            },
        }
    }
    let given_lbls: Vec<&String> = el
        .attrs
        .iter()
        .filter_map(|a| match a {
            JsxAttr::Attr(_, name, Some(_)) => Some(name),
            _ => None,
        })
        .collect();
    for (l, _, opt) in &sig.props {
        if !opt && !given_lbls.contains(&l) {
            tc.report(
                Diagnostic::error(
                    "E3034",
                    "MISSING ATTRIBUTE",
                    tag_sp,
                    format!("{desc} needs `{l}`"),
                )
                .with_hints(vec![format!("Add `{l}={{…}}` to the tag.")]),
            );
        }
    }
    if el.has_close {
        for c in &el.children {
            infer_jsx_child(tc, cx, html_con, &this, c);
        }
        tc.report(
            Diagnostic::error(
                "E3037",
                "STATEFUL CHILDREN",
                el.span,
                format!("{desc} holds state — it can't wrap children yet"),
            )
            .with_hints(vec![format!("Self-close it: `<{} …/>`.", sig.name.text)]),
        );
    }
    match &sig.props_ctor {
        Some((ctor_name, ctor_ty)) => {
            // The Props constructor's type lands at the tag span, so Core
            // lowering reorders the labeled construction as usual.
            tc.expr_types.insert(tag_sp, ctor_ty.clone());
            let optionals: BTreeSet<String> = sig
                .props
                .iter()
                .filter(|(_, _, opt)| *opt)
                .map(|(l, _, _)| l.clone())
                .collect();
            tc.instances.insert(
                tag_sp,
                InstLower::Node(sig.name.clone(), Some((ctor_name.clone(), optionals))),
            );
        }
        None => {
            tc.instances
                .insert(tag_sp, InstLower::Node(sig.name.clone(), None));
        }
    }
    this
}

/// Check one `component` manifest against the wired functions'
/// (annotated) schemes and derive its `CompSig`. `None` when invalid
/// (E3036, already reported).
fn check_component(
    tc: &mut Tc,
    cx: &Cx,
    assumed: &BTreeMap<Unique, Scheme>,
    cd: &ComponentDef<Name>,
) -> Option<CompSig> {
    fn bad(tc: &mut Tc, sp: Span, msg: String) -> Option<CompSig> {
        tc.report(
            Diagnostic::error("E3036", "INVALID COMPONENT", sp, msg).with_hints(vec![
                "The shapes: `init: () -> Model` or `(Props) -> Model` with Props a record; `update: (Model, Msg) -> Model` (or returning a `Step(Model, List<Out>)`-shaped type); `view: (Props, Model) -> Html<Msg>` (just `(Model)` without props)."
                    .to_string(),
            ]),
        );
        None
    }
    let cname = &cd.name.text;
    let ty_cons = &cx.env.ty_cons;
    let scheme_of = |tc: &mut Tc, what: &str, w: &(Span, Name)| -> Option<(Span, Scheme)> {
        let (sp, n) = w;
        if n.sort == NameSort::Error {
            return None; // unknown wired name: already reported
        }
        match assumed.get(&n.unique) {
            Some(sch) => Some((*sp, sch.clone())),
            None => {
                bad(
                    tc,
                    *sp,
                    format!(
                        "`{what}:` must wire a fully annotated function — the manifest is read off signatures"
                    ),
                );
                None
            }
        }
    };
    let option_payload = |ty: &Type| -> Option<Type> {
        match ty {
            Type::Con(c, inner) if inner.len() == 1 => match ty_cons.get(&c.name.unique) {
                Some(ti) => {
                    let ns: Vec<&str> = ti.ctors.iter().map(|c2| c2.name.text.as_str()).collect();
                    if ns.len() == 2 && ns.contains(&"Some") && ns.contains(&"None") {
                        Some(inner[0].clone())
                    } else {
                        None
                    }
                }
                None => None,
            },
            _ => None,
        }
    };
    let stateless = Some(CompSig {
        name: cd.name.clone(),
        view: cd.view.1.clone(),
        stateful: false,
        props: Vec::new(),
        props_ctor: None,
        step: false,
        outs: Vec::new(),
    });

    let mview = scheme_of(tc, "view", &cd.view);
    match (&cd.init, &cd.update, mview) {
        (_, _, None) => None,
        (None, Some((usp, _)), _) => bad(
            tc,
            *usp,
            format!(
                "`{cname}` wires an `update` but no `init` — a stateless component wires only `view`"
            ),
        ),
        (None, None, Some(_)) => stateless,
        (Some((isp, _)), None, _) => bad(
            tc,
            *isp,
            format!(
                "`{cname}` wires an `init` but no `update` — state without messages could never change"
            ),
        ),
        (Some(init_w), Some(update_w), Some((vsp, view_sch))) => {
            let minit = scheme_of(tc, "init", init_w);
            let mupdate = scheme_of(tc, "update", update_w);
            let (Some((isp, init_sch)), Some((usp, update_sch))) = (minit, mupdate) else {
                return None;
            };
            if !init_sch.vars.is_empty() || !update_sch.vars.is_empty() {
                return bad(
                    tc,
                    cd.span,
                    format!(
                        "`{cname}`'s wired functions must be monomorphic — generic components don't exist yet"
                    ),
                );
            }
            if !view_sch.vars.is_empty() {
                return bad(
                    tc,
                    vsp,
                    "a stateful component's `view:` must be monomorphic — its Msg is concrete"
                        .to_string(),
                );
            }
            let init_ty = &init_sch.ty;
            let update_ty = &update_sch.ty;
            let view_ty = &view_sch.ty;
            let Type::Fun(ips, model_ty) = init_ty else {
                return bad(
                    tc,
                    isp,
                    "`init:` must be a function returning the Model".to_string(),
                );
            };
            let model_ty = &**model_ty;
            // init: () -> Model, or (Props) -> Model with Props a record
            let mprops: Option<Option<(Type, CtorInfo, Vec<String>)>> = match &ips[..] {
                [] => Some(None),
                [
                    TParam {
                        ty: p_ty @ Type::Con(p_con, args),
                        ..
                    },
                ] if args.is_empty() => match ty_cons.get(&p_con.name.unique) {
                    Some(ti) if ti.ctors.len() == 1 && ti.ctors[0].fields.is_some() => {
                        let ctor = ti.ctors[0].clone();
                        let lbls = ctor.fields.clone().unwrap();
                        Some(Some((p_ty.clone(), ctor, lbls)))
                    }
                    _ => {
                        bad(
                            tc,
                            isp,
                            "`init:` takes nothing, or one record of props".to_string(),
                        );
                        None
                    }
                },
                _ => {
                    bad(
                        tc,
                        isp,
                        "`init:` takes nothing, or one record of props".to_string(),
                    );
                    None
                }
            };
            let mprops = mprops?;
            let Type::Fun(ups, ret_ty) = update_ty else {
                return bad(
                    tc,
                    usp,
                    "`update:` is `(Model, Msg) -> Model` (or `-> Step`)".to_string(),
                );
            };
            let [TParam { ty: m1, .. }, TParam { ty: msg_ty, .. }] = &ups[..] else {
                return bad(
                    tc,
                    usp,
                    "`update:` is `(Model, Msg) -> Model` (or `-> Step`)".to_string(),
                );
            };
            if !same_type(m1, model_ty) {
                return bad(
                    tc,
                    usp,
                    "`update:`'s first parameter must be `init:`'s Model".to_string(),
                );
            }
            let ret_ty = &**ret_ty;
            let step_info: Option<(bool, Vec<(String, Vec<Type>)>)> = if same_type(ret_ty, model_ty)
            {
                Some((false, Vec::new()))
            } else {
                match ret_ty {
                    Type::Con(s_con, args) if args.is_empty() => {
                        let ok = ty_cons.get(&s_con.name.unique).and_then(|ti| {
                            let [sc] = &ti.ctors[..] else { return None };
                            let [a, b] = &sc.args[..] else { return None };
                            if !same_type(a, model_ty) {
                                return None;
                            }
                            let Type::Con(lc, l_args) = b else {
                                return None;
                            };
                            if *lc != tc_list() {
                                return None;
                            }
                            let [Type::Con(oc, o_args)] = &l_args[..] else {
                                return None;
                            };
                            if !o_args.is_empty() {
                                return None;
                            }
                            let oti = ty_cons.get(&oc.name.unique)?;
                            Some(
                                oti.ctors
                                    .iter()
                                    .map(|c| (lower_first_t(&c.name.text), c.args.clone()))
                                    .collect::<Vec<_>>(),
                            )
                        });
                        match ok {
                            Some(outs) => Some((true, outs)),
                            None => {
                                bad(
                                    tc,
                                    usp,
                                    "`update:` returns its Model, or a `Step(Model, List<Out>)`-shaped type"
                                        .to_string(),
                                );
                                None
                            }
                        }
                    }
                    _ => {
                        bad(
                            tc,
                            usp,
                            "`update:` returns its Model, or a `Step(Model, List<Out>)`-shaped type"
                                .to_string(),
                        );
                        None
                    }
                }
            };
            let (step, outs) = step_info?;
            let ret_is_html = |vret: &Type| -> bool {
                match (&cx.env.jsx_html, vret) {
                    (Some(hc), Type::Con(c, v_args)) if v_args.len() == 1 => {
                        c == hc && same_type(&v_args[0], msg_ty)
                    }
                    _ => false,
                }
            };
            let view_ok = match (&mprops, view_ty) {
                (Some((p_ty, _, _)), Type::Fun(vps, vret)) if vps.len() == 2 => {
                    same_type(&vps[0].ty, p_ty)
                        && same_type(&vps[1].ty, model_ty)
                        && ret_is_html(vret)
                }
                (None, Type::Fun(vps, vret)) if vps.len() == 1 => {
                    same_type(&vps[0].ty, model_ty) && ret_is_html(vret)
                }
                _ => false,
            };
            if !view_ok {
                return bad(
                    tc,
                    vsp,
                    format!(
                        "`view:` must be `({}Model) -> Html<Msg>` — the same Props, Model, and Msg the other wires use",
                        if mprops.is_some() { "Props, " } else { "" }
                    ),
                );
            }
            Some(CompSig {
                name: cd.name.clone(),
                view: cd.view.1.clone(),
                stateful: true,
                props: mprops
                    .as_ref()
                    .map(|(_, ctor, lbls)| {
                        lbls.iter()
                            .zip(&ctor.args)
                            .map(|(l, t)| {
                                let payload = option_payload(t);
                                (
                                    l.clone(),
                                    payload.clone().unwrap_or_else(|| t.clone()),
                                    payload.is_some(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                props_ctor: mprops.as_ref().map(|(p_ty, ctor, lbls)| {
                    (
                        ctor.name.clone(),
                        Type::Fun(
                            lbls.iter()
                                .zip(&ctor.args)
                                .map(|(l, t)| TParam {
                                    label: Some(l.clone()),
                                    ty: t.clone(),
                                })
                                .collect(),
                            Box::new(p_ty.clone()),
                        ),
                    )
                }),
                step,
                outs,
            })
        }
    }
}

fn lower_first_t(t: &str) -> String {
    let mut cs = t.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

/// Structural equality on zonked, meta-free types (manifest schemes are
/// fully annotated). Labels don't count, as everywhere.
fn same_type(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Con(c1, as1), Type::Con(c2, as2)) => {
            c1 == c2 && as1.len() == as2.len() && as1.iter().zip(as2).all(|(x, y)| same_type(x, y))
        }
        (Type::Var(v1), Type::Var(v2)) => v1 == v2,
        (Type::Fun(ps1, r1), Type::Fun(ps2, r2)) => {
            ps1.len() == ps2.len()
                && ps1.iter().zip(ps2).all(|(p, q)| same_type(&p.ty, &q.ty))
                && same_type(r1, r2)
        }
        (Type::Error, Type::Error) => true,
        _ => false,
    }
}

fn lit_type(tc: &mut Tc, cx: &Cx, l: &Lit) -> Type {
    match l {
        Lit::Int(_) => tc.fresh_meta(cx, Flavor::Num),
        Lit::Float(_) => t_float(),
        Lit::Str(_) => t_string(),
    }
}

/// `expr?`: decide Result vs Option (from the operand's head, or
/// bidirectionally from the enclosing return type), unwrap the success
/// payload, and require the failure to flow into the enclosing return.
fn infer_try(
    tc: &mut Tc,
    cx: &Cx,
    sp: Span,
    q_sp: Span,
    operand_sp: Span,
    inner_ty: &Type,
) -> Type {
    let t = tc.arena.zonk(inner_ty);
    let Some(rc) = cx.ret.clone() else {
        tc.report(
            Diagnostic::error(
                "E3021",
                "? OUTSIDE A FUNCTION",
                q_sp,
                "there is no function for `?` to return from",
            )
            .with_hints(vec![
                "`?` early-returns from the nearest `fn` or lambda; a top-level `let` has neither."
                    .to_string(),
            ]),
        );
        return Type::Error;
    };
    let ret_z = tc.arena.zonk(&rc.ty);
    let kind_of = |env: &TcEnv, ty: &Type| -> Option<(TryKind, TyCon)> {
        match ty {
            Type::Con(c, _) => {
                if Some(c) == resolve_try_con(env, TryKind::Result).as_ref() {
                    Some((TryKind::Result, c.clone()))
                } else if Some(c) == resolve_try_con(env, TryKind::Option).as_ref() {
                    Some((TryKind::Option, c.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    };
    match &t {
        Type::Error => Type::Error,
        _ if kind_of(&cx.env, &t).is_some() => {
            let (kind, con) = kind_of(&cx.env, &t).unwrap();
            proceed(tc, cx, sp, q_sp, &rc, kind, con, &ret_z, inner_ty)
        }
        Type::Meta(_) => match kind_of(&cx.env, &ret_z) {
            // The operand is undetermined: let the enclosing return type
            // decide (so `fn f(x) -> Result<e, a> { Ok(x? + 1) }` infers
            // x's type from the signature).
            Some((kind, con)) => proceed(tc, cx, sp, q_sp, &rc, kind, con, &ret_z, inner_ty),
            None => match &ret_z {
                Type::Meta(_) => {
                    tc.report(
                        Diagnostic::error(
                            "E3024",
                            "AMBIGUOUS ?",
                            q_sp,
                            "I can't tell whether this `?` unwraps a `Result` or an `Option`",
                        )
                        .with_hints(vec![
                            "Annotate the operand or the enclosing function's return type."
                                .to_string(),
                        ]),
                    );
                    Type::Error
                }
                Type::Error => Type::Error,
                _ => {
                    conflicting_return(tc, q_sp, &rc, &ret_z, None);
                    Type::Error
                }
            },
        },
        _ => {
            let tr = tc.render(&t);
            tc.report(
                Diagnostic::error(
                    "E3022",
                    "CANNOT UNWRAP",
                    operand_sp,
                    format!("this is `{tr}`"),
                )
                .with_body(vec![dtext(format!(
                    "`?` unwraps a `Result` or an `Option`, returning the failure to the caller — but this is `{tr}`."
                ))]),
            );
            Type::Error
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn proceed(
    tc: &mut Tc,
    cx: &Cx,
    sp: Span,
    q_sp: Span,
    rc: &RetCx,
    kind: TryKind,
    con: TyCon,
    ret_z: &Type,
    inner_ty: &Type,
) -> Type {
    let payload = tc.fresh_meta(cx, Flavor::Star);
    let rho = tc.fresh_meta(cx, Flavor::Star);
    let (op_want, ret_want) = match kind {
        TryKind::Result => {
            let err_ty = tc.fresh_meta(cx, Flavor::Star);
            (
                Type::Con(con.clone(), vec![err_ty.clone(), payload.clone()]),
                Type::Con(con.clone(), vec![err_ty, rho]),
            )
        }
        TryKind::Option => (
            Type::Con(con.clone(), vec![payload.clone()]),
            Type::Con(con.clone(), vec![rho]),
        ),
    };
    tc.unify_r(&Reason::TryOperand(q_sp), inner_ty, &op_want);
    let kind_of_head = |env: &TcEnv, c: &TyCon| -> Option<TryKind> {
        if Some(c) == resolve_try_con(env, TryKind::Result).as_ref() {
            Some(TryKind::Result)
        } else if Some(c) == resolve_try_con(env, TryKind::Option).as_ref() {
            Some(TryKind::Option)
        } else {
            None
        }
    };
    match ret_z {
        // A return type with a known, incompatible head gets the direct
        // conflict diagnostic (with the annotation in view) instead of a
        // generic mismatch.
        Type::Con(c, _) if kind_of_head(&cx.env, c) == Some(kind) => {
            tc.unify_r(&Reason::TryRet(rc.name.clone(), q_sp), &ret_want, &rc.ty);
        }
        Type::Meta(_) => {
            tc.unify_r(&Reason::TryRet(rc.name.clone(), q_sp), &ret_want, &rc.ty);
        }
        Type::Error => {}
        _ => conflicting_return(tc, q_sp, rc, ret_z, Some(kind)),
    }
    tc.try_kinds.insert(sp, kind);
    payload
}

fn conflicting_return(tc: &mut Tc, q_sp: Span, rc: &RetCx, ret_z: &Type, m_kind: Option<TryKind>) {
    let ret_r = tc.render(ret_z);
    let fail_word = match m_kind {
        Some(TryKind::Result) => "a `Result` failure",
        Some(TryKind::Option) => "an `Option` failure",
        None => "a failure",
    };
    let fn_ref = match &rc.name {
        Some(n) => format!("`{n}`"),
        None => "the enclosing function".to_string(),
    };
    let secondary = match &rc.name {
        Some(_) => vec![(rc.span, format!("but the return type here is `{ret_r}`"))],
        None => Vec::new(),
    };
    tc.report(
        Diagnostic::error(
            "E3023",
            "? RETURN CONFLICT",
            q_sp,
            format!("this `?` early-returns {fail_word} to the caller"),
        )
        .with_secondary(secondary)
        .with_body(vec![dtext(format!(
            "For the failure to escape, {fn_ref} must return a `Result` or an `Option` — but it returns `{ret_r}`."
        ))]),
    );
}

/// The in-scope `Result`/`Option` type constructor — resolved through
/// the same namespace annotations use (so shadowing behaves identically),
/// and only when its shape is exactly the standard one, which is what
/// guarantees the emitted tag test and payload field are right.
pub fn resolve_try_con(env: &TcEnv, kind: TryKind) -> Option<TyCon> {
    let ty_name = match kind {
        TryKind::Result => "Result",
        TryKind::Option => "Option",
    };
    let n = env.type_names.get(ty_name)?;
    let ti = env.ty_cons.get(&n.unique)?;
    let payload_is = |v: &TyVar, args: &[Type]| -> bool {
        matches!(args, [Type::Var(v2)] if v2.unique == v.unique)
    };
    let ctor_with = |nm: &str, p: &dyn Fn(&[Type]) -> bool| -> bool {
        ti.ctors.iter().any(|c| c.name.text == nm && p(&c.args))
    };
    let shape_ok = match (kind, &ti.params[..]) {
        (TryKind::Result, [e, a]) => {
            ti.ctors.len() == 2
                && ctor_with("Ok", &|args| payload_is(a, args))
                && ctor_with("Err", &|args| payload_is(e, args))
        }
        (TryKind::Option, [a]) => {
            ti.ctors.len() == 2
                && ctor_with("Some", &|args| payload_is(a, args))
                && ctor_with("None", &|args| args.is_empty())
        }
        _ => false,
    };
    if shape_ok { Some(ti.con.clone()) } else { None }
}

/// A bare constructor reference: its function type (or the value itself
/// when nullary).
fn con_ref_type(tc: &mut Tc, cx: &Cx, _sp: Span, n: &Name) -> Type {
    if n.sort == NameSort::Error {
        return Type::Error;
    }
    match cx.env.ctors.get(&n.unique).cloned() {
        None => Type::Error,
        Some(ci) => {
            let (args, result) = instantiate_ctor(tc, cx, &ci);
            // A constructor is a function whose labels are its field
            // names (positional ADT payloads stay unlabeled).
            if args.is_empty() {
                result
            } else {
                let params: Vec<TParam> = match &ci.fields {
                    Some(fields) => fields
                        .iter()
                        .zip(&args)
                        .map(|(l, t)| TParam {
                            label: Some(l.clone()),
                            ty: t.clone(),
                        })
                        .collect(),
                    None => args
                        .iter()
                        .map(|t| TParam {
                            label: None,
                            ty: t.clone(),
                        })
                        .collect(),
                };
                Type::Fun(params, Box::new(result))
            }
        }
    }
}

fn instantiate_ctor(tc: &mut Tc, cx: &Cx, ci: &CtorInfo) -> (Vec<Type>, Type) {
    let mut sub: BTreeMap<Unique, Type> = BTreeMap::new();
    let mut inst_params: Vec<Type> = Vec::with_capacity(ci.params.len());
    for v in &ci.params {
        let m = tc.fresh_meta(cx, v.flavor);
        sub.insert(v.unique, m.clone());
        inst_params.push(m);
    }
    (
        ci.args.iter().map(|a| subst_ty_vars(&sub, a)).collect(),
        Type::Con(ci.ty_con.clone(), inst_params),
    )
}

/// Branch joins return the first branch's type object, which would also
/// make its parameter LABELS win. Labels are advisory: where two branches
/// disagree at the same (top-level) position, drop the label instead of
/// letting one branch name the other's parameters. Nested occurrences
/// keep the first branch's labels — the spec words labels as advisory.
fn strip_disagreeing_labels(tc: &mut Tc, first: &Type, others: &[Type]) -> Type {
    let f = tc.arena.zonk(first);
    let os: Vec<Type> = others.iter().map(|o| tc.arena.zonk(o)).collect();
    match f {
        Type::Fun(ps, r) => {
            let agrees = |o: &Type| matches!(o, Type::Fun(qs, _) if qs.len() == ps.len());
            if os.iter().all(agrees) {
                let merged = os.iter().fold(ps, |acc, o| {
                    let Type::Fun(qs, _) = o else { return acc };
                    acc.into_iter()
                        .zip(qs)
                        .map(|(p, q)| {
                            if p.label == q.label {
                                p
                            } else {
                                TParam {
                                    label: None,
                                    ty: p.ty,
                                }
                            }
                        })
                        .collect()
                });
                Type::Fun(merged, r)
            } else {
                Type::Fun(ps, r)
            }
        }
        other => other,
    }
}

// Casts -----------------------------------------------------------------------

/// Resolve one `cast` declaration's signature: identity and home-rule
/// checks here, duplicates in the caller (which sees the whole table).
/// Returns the table key and entry; `None` when the declaration is
/// invalid (already reported).
fn check_cast_sig(
    tc: &mut Tc,
    cx: &Cx,
    here: &ModuleName,
    cd: &CastDef<Name>,
) -> Option<((Unique, Unique), (Name, Scheme))> {
    let mut anns: Vec<&TypeAnn> = cd.param.ann.as_ref().into_iter().collect();
    anns.push(&cd.ret);
    let scope = rigid_scope_for(tc, &anns);
    let cx2 = Cx {
        rigids: scope.clone(),
        ..cx.clone()
    };
    let src_ty = match &cd.param.ann {
        Some(a) => resolve_ann(tc, &cx2, a),
        None => Type::Error,
    };
    let tgt_ty = resolve_ann(tc, &cx2, &cd.ret);
    let (Type::Con(src, _), Type::Con(tgt, _)) = (src_ty.clone(), tgt_ty.clone()) else {
        // An annotation failed to resolve (E3001/E3011 already reported).
        return None;
    };
    let (src, tgt) = (&src, &tgt);
    if src == tgt {
        tc.report(
            Diagnostic::error(
                "E3032",
                "IDENTITY CAST",
                cd.span,
                format!("this casts `{}` to itself", cd.src_head),
            )
            .with_hints(vec![
                "`::` already treats a same-type cast as the identity — delete this declaration."
                    .to_string(),
            ]),
        );
        return None;
    }
    let builtin = |c: &TyCon| c.name.unique.is_builtin();
    let defining_module = |c: &TyCon| -> Option<ModuleName> {
        match &c.name.sort {
            NameSort::TyCon(m) => Some(m.clone()),
            _ => None,
        }
    };
    let home = if builtin(tgt) {
        if builtin(src) {
            None
        } else {
            defining_module(src)
        }
    } else {
        defining_module(tgt)
    };
    match home {
        None => {
            tc.report(Diagnostic::error(
                "E3031",
                "MISPLACED CAST",
                cd.span,
                "casts between built-in types are wired into the language",
            ));
            None
        }
        Some(home) if home != *here => {
            tc.report(
                Diagnostic::error(
                    "E3031",
                    "MISPLACED CAST",
                    cd.span,
                    format!("this cast belongs in `{}`", home.0),
                )
                .with_hints(vec![
                    "A cast lives with the type it involves — its target's module (its source's, when the target is built in)."
                        .to_string(),
                ]),
            );
            None
        }
        Some(_) => {
            let label = cd.param.name.as_ref().map(|n| n.text.clone());
            let sch = Scheme {
                vars: scope.values().cloned().collect(),
                ty: Type::Fun(
                    vec![TParam {
                        label,
                        ty: src_ty.clone(),
                    }],
                    Box::new(tgt_ty),
                ),
            };
            Some(((src.name.unique, tgt.name.unique), (cd.name.clone(), sch)))
        }
    }
}

/// Check a `cast` declaration's body against its (already-resolved)
/// signature, like a fully-annotated fn.
fn check_cast_body(tc: &mut Tc, cx: &Cx, cd: &CastDef<Name>, sch: &Scheme) {
    let Type::Fun(ps, ret) = &sch.ty else { return };
    let [p] = &ps[..] else { return };
    let scope: BTreeMap<String, TyVar> = sch
        .vars
        .iter()
        .map(|v| (v.text.clone(), v.clone()))
        .collect();
    let mut cx_body = match &cd.param.name {
        Some(n) => bind_val(n, mono_scheme(p.ty.clone()), cx),
        None => cx.clone(),
    };
    cx_body.rigids = scope;
    cx_body.level = cx.level.bump();
    cx_body.ret = Some(RetCx {
        ty: (**ret).clone(),
        name: None,
        span: cd.span,
    });
    let display = format!("cast ({}) -> {}", cd.src_head, cd.tgt_head);
    let body_ty = infer_expr(tc, &cx_body, &cd.body);
    tc.unify_r(&Reason::RetAnn(display, cd.body.span()), &body_ty, ret);
}

/// `e::T` — look the conversion up in the cast table by head pair. A
/// same-head cast is the identity. The resolved function lands in
/// `cast_fns` for Core lowering, and the cast's type is the conversion's
/// (instantiated) return type — so a cast to a generic type gets its
/// arguments from the conversion.
fn infer_cast(tc: &mut Tc, cx: &Cx, sp: Span, e: &Expr<Name>, ty_sp: Span, ty_txt: &str) -> Type {
    let src_ty0 = infer_expr(tc, cx, e);
    // A still-numeric literal source means Int: `1::Float` is Int→Float.
    let src_z = tc.arena.zonk(&src_ty0);
    let src_ty = match &src_z {
        Type::Meta(m) if tc.arena.flavor(*m) == Flavor::Num => {
            tc.unify_r(
                &Reason::CallArg(1, "the cast".to_string(), e.span()),
                &src_z,
                &t_int(),
            );
            t_int()
        }
        t => t.clone(),
    };
    match cx.env.type_names.get(ty_txt).cloned() {
        None => {
            let cands: Vec<String> = cx.env.type_names.keys().cloned().collect();
            tc.report(
                Diagnostic::error(
                    "E3001",
                    "UNKNOWN TYPE",
                    ty_sp,
                    format!("I don't know a type named `{ty_txt}`"),
                )
                .with_hints(did_you_mean(ty_txt, &cands)),
            );
            Type::Error
        }
        Some(ty_name) => match cx.env.ty_cons.get(&ty_name.unique).cloned() {
            None => Type::Error,
            Some(ti) => cast_to(tc, cx, sp, e, &ti.con, &src_ty),
        },
    }
}

fn cast_to(tc: &mut Tc, cx: &Cx, sp: Span, e: &Expr<Name>, target: &TyCon, src_ty: &Type) -> Type {
    match src_ty {
        Type::Error => Type::Error,
        Type::Con(src, _) => {
            // Same head constructor: the cast is the identity.
            if src == target {
                return src_ty.clone();
            }
            match cx
                .env
                .casts
                .get(&(src.name.unique, target.name.unique))
                .cloned()
            {
                None => {
                    let src_txt = &src.name.text;
                    let target_txt = &target.name.text;
                    let hint = if src.name.unique.is_builtin() && target.name.unique.is_builtin() {
                        "Casts between built-in types are wired into the language.".to_string()
                    } else {
                        format!(
                            "Declare `cast (x: {src_txt}) -> {target_txt} {{ … }}` in the module that defines {}.",
                            if target.name.unique.is_builtin() {
                                format!("`{src_txt}`")
                            } else {
                                format!("`{target_txt}`")
                            }
                        )
                    };
                    tc.report(
                        Diagnostic::error(
                            "E3028",
                            "NO SUCH CAST",
                            sp,
                            format!("no cast from `{src_txt}` to `{target_txt}` is declared"),
                        )
                        .with_hints(vec![hint]),
                    );
                    Type::Error
                }
                Some((fn_name, sch)) => {
                    let inst = instantiate(tc, cx, &sch);
                    let inst_z = tc.arena.zonk(&inst);
                    match inst_z {
                        Type::Fun(ps, ret) if ps.len() == 1 => {
                            tc.unify_r(
                                &Reason::CallArg(
                                    1,
                                    format!("the cast to `{}`", target.name.text),
                                    e.span(),
                                ),
                                src_ty,
                                &ps[0].ty,
                            );
                            tc.cast_fns.insert(sp, fn_name);
                            *ret
                        }
                        _ => Type::Error, // table entries are 1-ary by construction
                    }
                }
            }
        }
        _ => {
            let src_r = tc.render(src_ty);
            tc.report(
                Diagnostic::error(
                    "E3027",
                    "UNRESOLVED CAST SOURCE",
                    e.span(),
                    format!(
                        "I can't tell what type this is yet (it looks like `{src_r}`), so I can't pick a conversion"
                    ),
                )
                .with_hints(vec![
                    "Annotate the value so the cast knows its source type.".to_string(),
                ]),
            );
            Type::Error
        }
    }
}

// Calls -----------------------------------------------------------------------

enum PreArg<'a> {
    Pre(Type, Span),
    Arg(&'a Arg<Name>),
}

fn infer_call(tc: &mut Tc, cx: &Cx, sp: Span, callee: &Expr<Name>, pre_args: &[PreArg]) -> Type {
    let is_labeled = |pa: &PreArg| matches!(pa, PreArg::Arg(Arg::Lab(_, _, _)));
    let arg_ty = |tc: &mut Tc, pa: &PreArg| -> (Type, Span) {
        match pa {
            PreArg::Pre(t, s) => (t.clone(), *s),
            PreArg::Arg(a) => {
                let t = infer_expr(tc, cx, a.expr());
                (t, a.expr().span())
            }
        }
    };
    let drain_all = |tc: &mut Tc| {
        for pa in pre_args {
            arg_ty(tc, pa);
        }
    };

    // Record constructors get field-flavored wording; everything else is
    // a plain function whose parameter names came along in its type.
    let record_ctor: Option<String> = match callee {
        Expr::Con(_, n) => match cx.env.ctors.get(&n.unique) {
            Some(ci) if ci.fields.is_some() => Some(n.text.clone()),
            _ => None,
        },
        _ => None,
    };
    let desc = describe_callee(callee);

    // A pipeline subject followed by labeled arguments: the subject fills
    // the FIRST parameter (it never counts toward the all-or-none rule),
    // and the labels match the remaining parameters.
    if let [PreArg::Pre(subj_ty, subj_sp), rest @ ..] = pre_args {
        if !rest.is_empty() && rest.iter().all(is_labeled) {
            let f_ty0 = infer_expr(tc, cx, callee);
            let f_ty = tc.arena.zonk(&f_ty0);
            return match f_ty {
                Type::Error => {
                    drain_all(tc);
                    Type::Error
                }
                Type::Fun(ps, ret) if !ps.is_empty() => {
                    let p1 = &ps[0];
                    tc.unify_r(&Reason::CallArg(1, desc.clone(), *subj_sp), subj_ty, &p1.ty);
                    for pa in rest {
                        if let PreArg::Arg(Arg::Lab(_, lbl, _)) = pa {
                            if Some(lbl) == p1.label.as_ref() {
                                tc.report(Diagnostic::error(
                                    "E3025",
                                    "UNKNOWN PARAMETER",
                                    sp,
                                    format!("`{lbl}` is already filled by the pipeline subject"),
                                ));
                            }
                        }
                    }
                    let given: Vec<(&String, &Expr<Name>)> = rest
                        .iter()
                        .filter_map(|pa| match pa {
                            PreArg::Arg(Arg::Lab(_, lbl, e)) if Some(lbl) != p1.label.as_ref() => {
                                Some((lbl, e))
                            }
                            _ => None,
                        })
                        .collect();
                    labeled_call(
                        tc,
                        cx,
                        sp,
                        &desc,
                        record_ctor.as_deref(),
                        1,
                        &ps[1..],
                        &ret,
                        &given,
                    )
                }
                Type::Fun(_, _) => {
                    drain_all(tc);
                    tc.report(Diagnostic::error(
                        "E3005",
                        "WRONG NUMBER OF ARGUMENTS",
                        sp,
                        format!("{desc} expects 0 arguments, but the pipeline gives it one"),
                    ));
                    Type::Error
                }
                Type::Meta(m) if tc.arena.flavor(m) != Flavor::Num => {
                    unknown_params(tc, callee);
                    drain_all(tc);
                    Type::Error
                }
                other => {
                    not_a_function(tc, callee, &other);
                    drain_all(tc);
                    Type::Error
                }
            };
        }
    }
    if pre_args.iter().any(is_labeled) && !pre_args.iter().all(is_labeled) {
        tc.report(
            Diagnostic::error(
                "E3019",
                "MIXED ARGUMENTS",
                sp,
                "this call mixes labeled and positional arguments",
            )
            .with_hints(vec![
                "Either label every argument or none of them (a pipeline subject counts as positional)."
                    .to_string(),
            ]),
        );
        infer_expr(tc, cx, callee);
        drain_all(tc);
        return Type::Error;
    }
    let f_ty0 = infer_expr(tc, cx, callee);
    let f_ty = tc.arena.zonk(&f_ty0);
    let k = pre_args.len();
    match f_ty {
        Type::Error => {
            drain_all(tc);
            Type::Error
        }
        Type::Fun(ps, ret) => {
            if pre_args.iter().any(is_labeled) {
                let given: Vec<(&String, &Expr<Name>)> = pre_args
                    .iter()
                    .filter_map(|pa| match pa {
                        PreArg::Arg(Arg::Lab(_, lbl, e)) => Some((lbl, e)),
                        _ => None,
                    })
                    .collect();
                labeled_call(
                    tc,
                    cx,
                    sp,
                    &desc,
                    record_ctor.as_deref(),
                    0,
                    &ps,
                    &ret,
                    &given,
                )
            } else if ps.len() == k {
                for (i, (pa, want)) in pre_args.iter().zip(&ps).enumerate() {
                    let (got, gsp) = arg_ty(tc, pa);
                    tc.unify_r(&Reason::CallArg(i + 1, desc.clone(), gsp), &got, &want.ty);
                }
                *ret
            } else if k > 0 && k < ps.len() {
                // Fewer arguments than parameters is a PARTIAL
                // APPLICATION: the given args fill the prefix, the value
                // is a function of the remaining parameters (labels
                // kept). Zero args is not a partial — `f()` is always a
                // call-intent mistake.
                for (i, (pa, want)) in pre_args.iter().zip(&ps[..k]).enumerate() {
                    let (got, gsp) = arg_ty(tc, pa);
                    tc.unify_r(&Reason::CallArg(i + 1, desc.clone(), gsp), &got, &want.ty);
                }
                Type::Fun(ps[k..].to_vec(), ret)
            } else {
                drain_all(tc);
                tc.report(
                    Diagnostic::error(
                        "E3005",
                        "WRONG NUMBER OF ARGUMENTS",
                        sp,
                        format!(
                            "{desc} expects {}, but I see {k}",
                            count_text(ps.len(), "argument")
                        ),
                    )
                    .with_hints(if k == 0 {
                        vec!["To partially apply, pass at least one argument.".to_string()]
                    } else {
                        Vec::new()
                    }),
                );
                Type::Error
            }
        }
        Type::Meta(m) if tc.arena.flavor(m) == Flavor::Num => {
            drain_all(tc);
            tc.report(Diagnostic::error(
                "E3006",
                "NOT A FUNCTION",
                callee.span(),
                "this is a number, not something I can call",
            ));
            Type::Error
        }
        Type::Meta(_) => {
            if pre_args.iter().any(is_labeled) {
                drain_all(tc);
                unknown_params(tc, callee);
                Type::Error
            } else {
                let ps: Vec<Type> = pre_args
                    .iter()
                    .map(|_| tc.fresh_meta(cx, Flavor::Star))
                    .collect();
                let ret = tc.fresh_meta(cx, Flavor::Star);
                tc.unify_r(
                    &Reason::CallArg(1, desc.clone(), sp),
                    &f_ty,
                    &t_fun(ps.clone(), ret.clone()),
                );
                for (i, (pa, want)) in pre_args.iter().zip(&ps).enumerate() {
                    let (got, gsp) = arg_ty(tc, pa);
                    tc.unify_r(&Reason::CallArg(i + 1, desc.clone(), gsp), &got, want);
                }
                ret
            }
        }
        other => {
            drain_all(tc);
            let other_r = tc.render(&other);
            tc.report(Diagnostic::error(
                "E3006",
                "NOT A FUNCTION",
                callee.span(),
                format!("this is `{other_r}`, not something I can call"),
            ));
            Type::Error
        }
    }
}

fn not_a_function(tc: &mut Tc, callee: &Expr<Name>, other: &Type) {
    let other_r = tc.render(other);
    tc.report(Diagnostic::error(
        "E3006",
        "NOT A FUNCTION",
        callee.span(),
        format!("this is `{other_r}`, not something I can call"),
    ));
}

fn unknown_params(tc: &mut Tc, callee: &Expr<Name>) {
    tc.report(
        Diagnostic::error(
            "E3026",
            "UNKNOWN PARAMETERS",
            callee.span(),
            "I don't know the parameters of this function yet, so I can't match these labels",
        )
        .with_hints(vec![
            "Annotate it, like `f: (x: Int, y: Int) -> Int`, so I know the parameter names."
                .to_string(),
        ]),
    );
}

/// An all-labeled call against resolved parameters (idx0 already filled
/// by a pipeline subject, if any): match labels in any order, report
/// duplicates/unknowns, unify in WRITTEN order, and return the ret type
/// — or, when parameters remain unfilled, the PARTIAL APPLICATION's
/// function type (remaining parameters in declaration order, labels
/// kept; unnamed parameters always remain).
#[allow(clippy::too_many_arguments)]
fn labeled_call(
    tc: &mut Tc,
    cx: &Cx,
    sp: Span,
    desc: &str,
    record_ctor: Option<&str>,
    idx0: usize,
    ps: &[TParam],
    ret: &Type,
    given: &[(&String, &Expr<Name>)],
) -> Type {
    let plabels: Vec<&String> = ps.iter().filter_map(|p| p.label.as_ref()).collect();
    let by_label: BTreeMap<&String, (usize, &Type)> = ps
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.label.as_ref().map(|l| (l, (idx0 + i + 1, &p.ty))))
        .collect();
    let names: Vec<&String> = given.iter().map(|(l, _)| *l).collect();
    // `nub` first: a label written twice is a DUPLICATE, not an
    // "unknown" (the dups check below names it once).
    let mut nub_names: Vec<&String> = Vec::new();
    for n in &names {
        if !nub_names.contains(n) {
            nub_names.push(n);
        }
    }
    let extra: Vec<&String> = nub_names
        .iter()
        .filter(|n| !plabels.contains(n))
        .copied()
        .collect();
    // names \\ nub names: the 2nd+ occurrence of each label, in order.
    let mut dups: Vec<&String> = Vec::new();
    {
        let mut to_remove = nub_names.clone();
        for n in &names {
            if let Some(pos) = to_remove.iter().position(|m| m == n) {
                to_remove.remove(pos);
            } else {
                dups.push(n);
            }
        }
    }
    let remaining: Vec<TParam> = ps
        .iter()
        .filter(|p| match &p.label {
            Some(l) => !names.contains(&l),
            None => true,
        })
        .cloned()
        .collect();
    let (noun, dup_title) = match record_ctor {
        Some(_) => ("field", "DUPLICATE FIELD"),
        None => ("argument", "DUPLICATE ARGUMENT"),
    };
    for d in &dups {
        tc.report(Diagnostic::error(
            "E3018",
            dup_title,
            sp,
            format!("the {noun} `{d}` is given twice"),
        ));
    }
    for x in &extra {
        let cands: Vec<String> = plabels.iter().map(|l| (*l).clone()).collect();
        match record_ctor {
            Some(ctor) => {
                tc.report(
                    Diagnostic::error(
                        "E3017",
                        "UNKNOWN FIELD",
                        sp,
                        format!("`{ctor}` has no field `{x}`"),
                    )
                    .with_hints(did_you_mean(x, &cands)),
                );
            }
            None => {
                tc.report(
                    Diagnostic::error(
                        "E3025",
                        "UNKNOWN PARAMETER",
                        sp,
                        format!("{desc} has no parameter named `{x}`"),
                    )
                    .with_hints(did_you_mean(x, &cands)),
                );
            }
        }
    }
    for (lbl, e) in given {
        let t = infer_expr(tc, cx, e);
        if let Some((i, want)) = by_label.get(lbl) {
            match record_ctor {
                Some(ctor) => tc.unify_r(
                    &Reason::RecField((*lbl).clone(), ctor.to_string(), e.span()),
                    &t,
                    want,
                ),
                None => tc.unify_r(&Reason::CallArg(*i, desc.to_string(), e.span()), &t, want),
            }
        }
    }
    if !(extra.is_empty() && dups.is_empty()) {
        Type::Error
    } else if remaining.is_empty() {
        ret.clone()
    } else {
        Type::Fun(remaining, Box::new(ret.clone()))
    }
}

/// Record update: the base must already be the record, replacements
/// unify with the field types of the base's own instantiation — so an
/// update never changes a type parameter.
fn infer_update(
    tc: &mut Tc,
    cx: &Cx,
    sp: Span,
    n: &Name,
    base: &Expr<Name>,
    fields: &[(Span, String, Expr<Name>)],
) -> Type {
    let drain_all = |tc: &mut Tc| {
        infer_expr(tc, cx, base);
        for (_, _, e) in fields {
            infer_expr(tc, cx, e);
        }
    };
    if n.sort == NameSort::Error {
        drain_all(tc);
        return Type::Error;
    }
    let ci = cx.env.ctors.get(&n.unique).cloned();
    match ci {
        Some(ci) if ci.fields.is_some() => {
            let field_names = ci.fields.clone().unwrap();
            let names: Vec<&String> = fields.iter().map(|(_, l, _)| l).collect();
            let mut nub_names: Vec<&String> = Vec::new();
            for name in &names {
                if !nub_names.contains(name) {
                    nub_names.push(name);
                }
            }
            let extra: Vec<&String> = nub_names
                .iter()
                .filter(|x| !field_names.contains(**x))
                .copied()
                .collect();
            let mut dups: Vec<&String> = Vec::new();
            {
                let mut to_remove = nub_names.clone();
                for name in &names {
                    if let Some(pos) = to_remove.iter().position(|m| m == name) {
                        to_remove.remove(pos);
                    } else {
                        dups.push(name);
                    }
                }
            }
            for d in &dups {
                tc.report(Diagnostic::error(
                    "E3018",
                    "DUPLICATE FIELD",
                    sp,
                    format!("the field `{d}` is given twice"),
                ));
            }
            for x in &extra {
                tc.report(
                    Diagnostic::error(
                        "E3017",
                        "UNKNOWN FIELD",
                        sp,
                        format!("`{}` has no field `{x}`", n.text),
                    )
                    .with_hints(did_you_mean(x, &field_names)),
                );
            }
            let (arg_tys, result) = instantiate_ctor(tc, cx, &ci);
            let base_ty = infer_expr(tc, cx, base);
            tc.unify_r(
                &Reason::UpdateBase(n.text.clone(), base.span()),
                &base_ty,
                &result,
            );
            let field_ty: BTreeMap<&String, &Type> = field_names.iter().zip(&arg_tys).collect();
            for (_, lbl, e) in fields {
                let t = infer_expr(tc, cx, e);
                if let Some(want) = field_ty.get(lbl) {
                    tc.unify_r(
                        &Reason::RecField(lbl.clone(), n.text.clone(), e.span()),
                        &t,
                        want,
                    );
                }
            }
            if extra.is_empty() && dups.is_empty() {
                result
            } else {
                Type::Error
            }
        }
        _ => {
            tc.report(
                Diagnostic::error(
                    "E3020",
                    "NOT A RECORD",
                    sp,
                    format!("`{}` is not a record constructor", n.text),
                )
                .with_hints(vec![
                    "Only record constructors can be updated: `User(..u, age: 31)`.".to_string(),
                ]),
            );
            drain_all(tc);
            Type::Error
        }
    }
}

fn describe_callee(callee: &Expr<Name>) -> String {
    match callee {
        Expr::Var(_, n) => format!("`{}`", n.text),
        Expr::Con(_, n) => format!("`{}`", n.text),
        Expr::Field(_, _, _, f) => format!("`.{f}`"),
        _ => "this function".to_string(),
    }
}

// Binary operators ----------------------------------------------------------------

fn infer_bin(
    tc: &mut Tc,
    cx: &Cx,
    op: BinOp,
    _op_sp: Span,
    l: &Expr<Name>,
    r: &Expr<Name>,
) -> Type {
    let side = |tc: &mut Tc, e: &Expr<Name>, want: &Type| {
        let t = infer_expr(tc, cx, e);
        tc.unify_r(&Reason::BinOp(op, e.span()), &t, want);
    };
    use BinOp::*;
    match op {
        Add | Sub | Mul | Mod => {
            let m = tc.fresh_meta(cx, Flavor::Num);
            side(tc, l, &m);
            side(tc, r, &m);
            m
        }
        // `/` and `**` are not closed over Int (`7 / 2`, `2 ** -1`): the
        // operands are any one number type, the result is always Float.
        // Conversions back stay explicit (`Float.trunc` and friends).
        Div | Pow => {
            let m = tc.fresh_meta(cx, Flavor::Num);
            side(tc, l, &m);
            side(tc, r, &m);
            t_float()
        }
        // `~/` divides either flavor; the quotient is an Int.
        IntDiv => {
            let m = tc.fresh_meta(cx, Flavor::Num);
            side(tc, l, &m);
            side(tc, r, &m);
            t_int()
        }
        // Unreachable from the surface since `++` was removed (CST
        // lowering rejects it with E1110); interpolation types its
        // splices through RInterp, not here. Kept total rather than
        // making the function partial.
        Concat => {
            side(tc, l, &t_string());
            side(tc, r, &t_string());
            t_string()
        }
        Eq | Neq => {
            let lt = infer_expr(tc, cx, l);
            let rt = infer_expr(tc, cx, r);
            tc.unify_r(&Reason::BinOp(op, r.span()), &rt, &lt);
            t_bool()
        }
        Lt | Gt | Le | Ge => {
            let m = tc.fresh_meta(cx, Flavor::Num);
            side(tc, l, &m);
            side(tc, r, &m);
            t_bool()
        }
        And | Or => {
            side(tc, l, &t_bool());
            side(tc, r, &t_bool());
            t_bool()
        }
    }
}

// Patterns -------------------------------------------------------------------------

pub fn check_pat(tc: &mut Tc, cx: &Cx, pat: &Pat<Name>, ty: &Type) -> Vec<(Name, Type)> {
    match pat {
        Pat::Wild(_) => Vec::new(),
        Pat::Var(_, n) => vec![(n.clone(), ty.clone())],
        Pat::Lit(sp, l) => {
            let p_ty = lit_type(tc, cx, l);
            tc.unify_r(&Reason::Pattern(*sp), &p_ty, ty);
            Vec::new()
        }
        Pat::Ctor(sp, n, args) => {
            if n.sort == NameSort::Error {
                return args
                    .iter()
                    .flat_map(|p| check_pat(tc, cx, p, &Type::Error))
                    .collect();
            }
            match cx.env.ctors.get(&n.unique).cloned() {
                None => args
                    .iter()
                    .flat_map(|p| check_pat(tc, cx, p, &Type::Error))
                    .collect(),
                Some(ci) => {
                    let (arg_tys, result) = instantiate_ctor(tc, cx, &ci);
                    tc.unify_r(&Reason::Pattern(*sp), &result, ty);
                    if args.len() != arg_tys.len() {
                        tc.report(Diagnostic::error(
                            "E3012",
                            "WRONG PATTERN ARITY",
                            *sp,
                            format!(
                                "`{}` takes {}, but this pattern gives {}",
                                n.text,
                                count_text(arg_tys.len(), "argument"),
                                args.len()
                            ),
                        ));
                        args.iter()
                            .flat_map(|p| check_pat(tc, cx, p, &Type::Error))
                            .collect()
                    } else {
                        args.iter()
                            .zip(&arg_tys)
                            .flat_map(|(p, t)| check_pat(tc, cx, p, t))
                            .collect()
                    }
                }
            }
        }
        Pat::List(sp, elems, rest) => {
            let el = tc.fresh_meta(cx, Flavor::Star);
            tc.unify_r(&Reason::Pattern(*sp), &t_list(el.clone()), ty);
            let mut binds: Vec<(Name, Type)> = elems
                .iter()
                .flat_map(|p| check_pat(tc, cx, p, &el))
                .collect();
            if let Some(PatRest { name: Some(n), .. }) = rest {
                binds.push((n.clone(), t_list(el)));
            }
            binds
        }
    }
}

// Blocks ----------------------------------------------------------------------------

fn infer_block(tc: &mut Tc, cx: &Cx, stmts: &[Stmt<Name>], result: &Expr<Name>) -> Type {
    match stmts {
        [] => infer_expr(tc, cx, result),
        [s, rest @ ..] => match s {
            Stmt::Discard(e) => {
                infer_expr(tc, cx, e);
                infer_block(tc, cx, rest, result)
            }
            Stmt::Let(_, n, ann, e) => {
                let cx_rhs = Cx {
                    level: cx.level.bump(),
                    ..cx.clone()
                };
                let rhs_ty = infer_expr(tc, &cx_rhs, e);
                let scheme = match ann {
                    Some(a) => {
                        let ann_ty = resolve_ann(tc, cx, a);
                        tc.unify_r(&Reason::LetAnn(e.span()), &rhs_ty, &ann_ty);
                        mono_scheme(ann_ty)
                    }
                    None => generalize_at(tc, cx.level, &rhs_ty),
                };
                let cx2 = bind_val(n, scheme, cx);
                infer_block(tc, &cx2, rest, result)
            }
        },
    }
}

// Pending field accesses --------------------------------------------------------------

/// Try to discharge queued field constraints; keep the still-ambiguous
/// ones.
fn try_solve_pending(tc: &mut Tc, cx: &Cx) {
    let ps = std::mem::take(&mut tc.pending);
    let mut remaining = Vec::new();
    for pf in ps {
        if !solve_field(tc, cx, false, &pf) {
            remaining.push(pf);
        }
    }
    tc.pending = remaining;
}

/// End of a declaration: everything still ambiguous becomes an error.
fn flush_pending(tc: &mut Tc, cx: &Cx) {
    let ps = std::mem::take(&mut tc.pending);
    for pf in ps {
        solve_field(tc, cx, true, &pf);
    }
}

fn solve_field(tc: &mut Tc, cx: &Cx, is_final: bool, pf: &PendingField) -> bool {
    let rec = tc.arena.zonk(&pf.rec_ty);
    match rec {
        Type::Error => true,
        Type::Con(c, args) => match cx.env.ty_cons.get(&c.name.unique).cloned() {
            Some(ti) if ti.fields.is_some() => {
                let fields = ti.fields.clone().unwrap();
                match fields.iter().find(|(l, _)| *l == pf.label) {
                    Some((_, fty)) => {
                        let sub: BTreeMap<Unique, Type> = ti
                            .params
                            .iter()
                            .map(|v| v.unique)
                            .zip(args.iter().cloned())
                            .collect();
                        let want = subst_ty_vars(&sub, fty);
                        tc.unify_r(
                            &Reason::FieldAccess(pf.label.clone(), pf.span),
                            &pf.result,
                            &want,
                        );
                        true
                    }
                    None => {
                        let cands: Vec<String> = fields.iter().map(|(l, _)| l.clone()).collect();
                        tc.report(
                            Diagnostic::error(
                                "E3009",
                                "UNKNOWN FIELD",
                                pf.span,
                                format!("`{}` has no field `{}`", c.name.text, pf.label),
                            )
                            .with_hints(did_you_mean(&pf.label, &cands)),
                        );
                        true
                    }
                }
            }
            _ => {
                let c_r = tc.render(&Type::Con(c, args));
                tc.report(Diagnostic::error(
                    "E3020",
                    "NOT A RECORD",
                    pf.span,
                    format!("this is `{c_r}`, which has no fields"),
                ));
                true
            }
        },
        Type::Meta(_) => {
            if is_final {
                tc.report(
                    Diagnostic::error(
                        "E3010",
                        "AMBIGUOUS RECORD",
                        pf.span,
                        format!(
                            "I can't tell which record type this is; I only know it has a field `{}`",
                            pf.label
                        ),
                    )
                    .with_hints(vec![
                        "Add an annotation, like `(u: User)`, so I know which record this is."
                            .to_string(),
                    ]),
                );
                true
            } else {
                false
            }
        }
        other => {
            let o_r = tc.render(&other);
            tc.report(Diagnostic::error(
                "E3020",
                "NOT A RECORD",
                pf.span,
                format!("this is `{o_r}`, which has no fields"),
            ));
            true
        }
    }
}

// Small helpers ------------------------------------------------------------------------

pub fn did_you_mean(t: &str, cands: &[String]) -> Vec<String> {
    fn lev(a: &str, b: &str) -> i32 {
        if (a.chars().count() as i64 - b.chars().count() as i64).abs() > 2 {
            return 3;
        }
        fn go(k: i32, a: &[char], b: &[char]) -> i32 {
            if k < 0 {
                return 3;
            }
            match (a, b) {
                ([x, xs @ ..], [y, ys @ ..]) => {
                    if x == y {
                        go(k, xs, ys)
                    } else {
                        1 + [go(k - 1, xs, b), go(k - 1, a, ys), go(k - 1, xs, ys)]
                            .into_iter()
                            .min()
                            .unwrap()
                    }
                }
                (xs, []) => xs.len() as i32,
                ([], ys) => ys.len() as i32,
            }
        }
        let av: Vec<char> = a.chars().collect();
        let bv: Vec<char> = b.chars().collect();
        go(2, &av, &bv)
    }
    cands
        .iter()
        .filter(|c| c.as_str() != t && c.chars().count() > 1 && lev(t, c) <= 2)
        .take(1)
        .map(|c| format!("Did you mean `{c}`?"))
        .collect()
}

fn count_text(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests;
