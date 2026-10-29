//! Lower the renamed, type-checked AST to Core: desugar pipes into
//! calls, list literals into Cons/Nil, blocks into let/seq chains;
//! eta-expand bare constructors and intrinsics; reorder labeled record
//! construction to field order (binding temps when the written order
//! differs, preserving evaluation order); attach compiled decision trees
//! to matches.
//!
//! Runs only on diagnostic-free modules, so the impossible cases really
//! are impossible.

use std::collections::BTreeMap;

use crate::ast::*;
use crate::builtin as b;
use crate::core::{Core, CoreDecl, CoreModule, Intrinsic};
use crate::match_analysis::DTree;
use crate::maxim_panic;
use crate::names::{Name, NameSort};
use crate::span::Span;
use crate::types::env::{InstLower, TcEnv};
use crate::types::ty::{TParam, Type};
use crate::unique::Unique;

/// The stdlib functions JSX desugars to, resolved by the driver from
/// the compiled stdlib's exports. `None` only while the stdlib itself
/// compiles — stdlib modules don't use JSX.
#[derive(Clone, Debug)]
pub struct JsxApi {
    pub node: Name,
    pub text: Name,
    pub attr: Name,
    pub on: Name,
    pub on_text: Name,
    pub on_bool: Name,
    pub append: Name,
    /// The prelude Option constructors — Some/None fills for optional
    /// component props.
    pub some: Name,
    pub none: Name,
}

#[allow(clippy::too_many_arguments)]
pub fn lower_to_core(
    env: &TcEnv,
    trees: &BTreeMap<Span, DTree>,
    try_kinds: &BTreeMap<Span, TryKind>,
    expr_types: &BTreeMap<Span, Type>,
    cast_fns: &BTreeMap<Span, Name>,
    instances: &BTreeMap<Span, InstLower>,
    jsx_api: Option<&JsxApi>,
    m: &Module<Name>,
) -> CoreModule {
    let mut lw = Lower {
        env,
        trees,
        try_kinds,
        expr_types,
        cast_fns,
        instances,
        jsx_api,
        tmp: 0,
    };
    let mut decls = Vec::new();
    for d in &m.decls {
        match d {
            Decl::Type(_) => {}
            Decl::Fn(fnd) => {
                let body = lw.expr(&fnd.body);
                decls.push(CoreDecl::Fn(
                    fnd.name.clone(),
                    fnd.params.iter().map(|p| p.name.clone()).collect(),
                    body,
                ));
            }
            Decl::Let(_, n, _, e) => {
                let e2 = lw.expr(e);
                decls.push(CoreDecl::Let(n.clone(), e2));
            }
            // A cast emits as an ordinary exported function under its
            // synthesized name (`floatToCelsius`).
            Decl::Cast(cd) => {
                let body = lw.expr(&cd.body);
                decls.push(CoreDecl::Fn(
                    cd.name.clone(),
                    vec![cd.param.name.clone()],
                    body,
                ));
            }
            // A component emits as its manifest object.
            Decl::Component(cd) => {
                let step = env
                    .components
                    .get(&cd.name.unique)
                    .is_some_and(|s| s.step);
                let mut slots: Vec<(String, Name)> = Vec::new();
                if let Some((_, n)) = &cd.init {
                    slots.push(("init".to_string(), n.clone()));
                }
                if let Some((_, n)) = &cd.update {
                    slots.push(("update".to_string(), n.clone()));
                }
                slots.push(("view".to_string(), cd.view.1.clone()));
                decls.push(CoreDecl::Component(cd.name.clone(), slots, step));
            }
        }
    }
    CoreModule {
        name: m.name.clone(),
        decls,
    }
}

struct Lower<'a> {
    env: &'a TcEnv,
    trees: &'a BTreeMap<Span, DTree>,
    try_kinds: &'a BTreeMap<Span, TryKind>,
    expr_types: &'a BTreeMap<Span, Type>,
    cast_fns: &'a BTreeMap<Span, Name>,
    instances: &'a BTreeMap<Span, InstLower>,
    jsx_api: Option<&'a JsxApi>,
    // Temps use deep-negative uniques: no collision with the renamer
    // (non-negative) or the builtins (small negative).
    tmp: i64,
}

/// A deferred `let` wrapper from [`Lower::hoist`].
type Wrap = Option<(Name, Core)>;

fn apply_wraps(wraps: Vec<Wrap>, mut body: Core) -> Core {
    for w in wraps.into_iter().rev() {
        if let Some((t, e)) = w {
            body = Core::Let(t, Box::new(e), Box::new(body));
        }
    }
    body
}

impl Lower<'_> {
    fn fresh_tmp(&mut self) -> Name {
        let n = self.tmp;
        self.tmp += 1;
        Name {
            unique: Unique(-1000 - n),
            text: format!("$t{n}"),
            sort: NameSort::Local,
            def_span: b::builtin_span(),
        }
    }

    /// A fresh binder that KEEPS a surface name (a parameter label): the
    /// emitted closure params read as the declaration's
    /// (`(y) => f(x, y)`). The emitter's name allocator dedups collisions.
    fn fresh_named(&mut self, txt: &str) -> Name {
        let n = self.tmp;
        self.tmp += 1;
        Name {
            unique: Unique(-1000 - n),
            text: txt.to_string(),
            sort: NameSort::Local,
            def_span: b::builtin_span(),
        }
    }

    /// Bind a non-atomic Core to a temp. Partial application evaluates
    /// its given arguments immediately, and any `?` inside them must
    /// return from the ENCLOSING function — hoisting keeps those
    /// statements outside the closure the partial builds.
    fn hoist(&mut self, c: Core) -> (Wrap, Core) {
        match c {
            Core::Var(_) | Core::Lit(_) => (None, c),
            _ => {
                let t = self.fresh_tmp();
                (Some((t.clone(), c)), Core::Var(t))
            }
        }
    }

    fn expr(&mut self, e: &Expr<Name>) -> Core {
        match e {
            Expr::Var(_, n) => match intrinsic_of(n) {
                Some(i) => self.eta_intrinsic(i),
                None => Core::Var(n.clone()),
            },
            Expr::Con(_, n) => self.eta_con(n),
            Expr::Lit(_, l) => Core::Lit(l.clone()),
            Expr::Dot(_, _, _, _) => {
                maxim_panic!("lower_to_core: EDot survives renaming only on errors")
            }
            Expr::Field(_, e2, _, f) => Core::Field(Box::new(self.expr(e2)), f.clone()),
            Expr::Call(_, f, args) => self.call(f, args),
            Expr::Update(_, _, _, base, fields) => {
                let base2 = self.expr(base);
                let fields2 = fields
                    .iter()
                    .map(|(_, lbl, e2)| (lbl.clone(), self.expr(e2)))
                    .collect();
                Core::Update(Box::new(base2), fields2)
            }
            Expr::Pipe(_, style, l, r) => match (style, &**r) {
                (PipeStyle::Into, Expr::Call(_, f, args)) => {
                    let mut all = vec![Arg::Pos((**l).clone())];
                    all.extend(args.iter().cloned());
                    self.call(f, &all)
                }
                _ => self.call(r, &[Arg::Pos((**l).clone())]),
            },
            Expr::Bin(_, op, _, l, r) => {
                Core::Bin(*op, Box::new(self.expr(l)), Box::new(self.expr(r)))
            }
            Expr::Unary(_, op, e2) => Core::Un(*op, Box::new(self.expr(e2))),
            Expr::If(_, c, t, e2) => Core::If(
                Box::new(self.expr(c)),
                Box::new(self.expr(t)),
                Box::new(self.expr(e2)),
            ),
            Expr::Match(sp, scrut, arms) => {
                let scrut2 = self.expr(scrut);
                let arms2: Vec<(Option<Core>, Core)> = arms
                    .iter()
                    .map(|a| {
                        (
                            a.guard.as_ref().map(|g| self.expr(g)),
                            self.expr(&a.body),
                        )
                    })
                    .collect();
                match self.trees.get(sp) {
                    Some(tree) => Core::Match(Box::new(scrut2), tree.clone(), arms2),
                    None => maxim_panic!("lower_to_core: match without a compiled tree"),
                }
            }
            Expr::Lam(_, params, _, body) => Core::Lam(
                params.iter().map(|p| p.name.clone()).collect(),
                Box::new(self.expr(body)),
            ),
            Expr::List(_, elems, spread) => {
                let tl = match spread {
                    Some(e2) => self.expr(e2),
                    None => Core::Con(b::ctor_nil(), Vec::new()),
                };
                let es: Vec<Core> = elems.iter().map(|e2| self.expr(e2)).collect();
                es.into_iter()
                    .rev()
                    .fold(tl, |t, h| Core::Con(b::ctor_cons(), vec![h, t]))
            }
            Expr::Block(_, stmts, result) => {
                let mut out = self.expr(result);
                // Statements are lowered in source order in Haskell too
                // (mapM effects run left to right before the foldr
                // assembles); temp numbering follows source order.
                let lowered: Vec<(usize, Core, Option<Name>)> = stmts
                    .iter()
                    .enumerate()
                    .map(|(i, s)| match s {
                        Stmt::Let(_, n, _, e2) => (i, self.expr(e2), Some(n.clone())),
                        Stmt::Discard(e2) => (i, self.expr(e2), None),
                    })
                    .collect();
                for (_, c, n) in lowered.into_iter().rev() {
                    out = match n {
                        Some(n) => Core::Let(n, Box::new(c), Box::new(out)),
                        None => Core::Seq(Box::new(c), Box::new(out)),
                    };
                }
                out
            }
            Expr::Try(sp, _, e2) => match self.try_kinds.get(sp) {
                Some(kind) => Core::Try(*kind, Box::new(self.expr(e2))),
                None => maxim_panic!("lower_to_core: `?` without a checked kind"),
            },
            // A cast is a plain call of the conversion the checker
            // resolved (an intrinsic or a user function); no entry means
            // identity.
            Expr::Cast(sp, e2, ty_sp, _) => match self.cast_fns.get(sp).cloned() {
                Some(fn_name) => {
                    self.call(&Expr::Var(*ty_sp, fn_name), &[Arg::Pos((**e2).clone())])
                }
                None => self.expr(e2),
            },
            // Interpolation lowers to a left fold of BConcat (the surface
            // `++` is gone, but the Core primitive remains its
            // compilation target), so the emitted JS is the flat
            // `"a" + x + "b"`. A bare "${x}" is just x (its String type
            // was already enforced).
            Expr::Interp(_, parts) => {
                let cs: Vec<Core> = parts
                    .iter()
                    .map(|p| match p {
                        InterpPart::Str(t) => Core::Lit(Lit::Str(t.clone())),
                        InterpPart::Expr(e2) => self.expr(e2),
                    })
                    .collect();
                let mut it = cs.into_iter();
                match it.next() {
                    None => Core::Lit(Lit::Str(String::new())),
                    Some(first) => it.fold(first, |acc, c| {
                        Core::Bin(BinOp::Concat, Box::new(acc), Box::new(c))
                    }),
                }
            }
            // A JSX element desugars to the AST of the calls it means —
            // `Html.node(tag, [attrs], [children])`, a component tag to
            // its saturated labeled `Module.view` call — and lowers THAT:
            // list literals, labeled reordering, and evaluation order all
            // reuse the ordinary machinery.
            Expr::Jsx(_, el) => match self.jsx_api {
                Some(api) => {
                    let desugared = desugar_jsx(api, self.instances, el);
                    self.expr(&desugared)
                }
                None => {
                    maxim_panic!("lower_to_core: JSX before the stdlib Html/List exports exist")
                }
            },
            Expr::Instance(_, comp, mp, outs) => Core::Instance(
                comp.clone(),
                mp.as_ref().map(|p| Box::new(self.expr(p))),
                outs.iter()
                    .map(|(ev, h)| (ev.clone(), self.expr(h)))
                    .collect(),
            ),
        }
    }

    fn call(&mut self, f: &Expr<Name>, args: &[Arg<Name>]) -> Core {
        if let Expr::Var(_, n) = f {
            if let Some(i) = intrinsic_of(n) {
                if args.len() == i.arity() {
                    return Core::Intrinsic(
                        i,
                        args.iter().map(|a| self.expr(a.expr())).collect(),
                    );
                }
            }
        }
        if let Expr::Con(_, n) = f {
            return self.construct(&n.clone(), args);
        }
        let written: Vec<(&String, &Expr<Name>)> = args
            .iter()
            .filter_map(|a| match a {
                Arg::Lab(_, lbl, e) => Some((lbl, e)),
                Arg::Pos(_) => None,
            })
            .collect();
        if !written.is_empty() {
            let pos_args: Vec<&Expr<Name>> = args
                .iter()
                .filter_map(|a| match a {
                    Arg::Pos(e) => Some(e),
                    Arg::Lab(_, _, _) => None,
                })
                .collect();
            self.labeled_app(f, &pos_args, &written)
        } else {
            self.positional_app(f, args)
        }
    }

    /// A positional call: saturated is a plain CApp; FEWER args than the
    /// callee's (resolved) parameters is a partial application — the
    /// given args evaluate now, the value closes over the remainder.
    fn positional_app(&mut self, f: &Expr<Name>, args: &[Arg<Name>]) -> Core {
        if let Some(Type::Fun(ps, _)) = self.expr_types.get(&f.span()) {
            if args.len() < ps.len() {
                let ps = ps.clone();
                let f2 = self.expr(f);
                let (wrap_f, fv) = self.hoist(f2);
                let pairs: Vec<(Wrap, Core)> = args
                    .iter()
                    .map(|a| {
                        let c = self.expr(a.expr());
                        self.hoist(c)
                    })
                    .collect();
                let params: Vec<Name> = ps[args.len()..]
                    .iter()
                    .map(|p| self.param_name(p))
                    .collect();
                let mut app_args: Vec<Core> = pairs.iter().map(|(_, v)| v.clone()).collect();
                app_args.extend(params.iter().map(|p| Core::Var(p.clone())));
                let body = Core::App(Box::new(fv), app_args);
                let lam = Core::Lam(params.into_iter().map(Some).collect(), Box::new(body));
                let wrapped = apply_wraps(pairs.into_iter().map(|(w, _)| w).collect(), lam);
                return apply_wraps(vec![wrap_f], wrapped);
            }
        }
        let f2 = self.expr(f);
        let args2: Vec<Core> = args.iter().map(|a| self.expr(a.expr())).collect();
        Core::App(Box::new(f2), args2)
    }

    fn param_name(&mut self, p: &TParam) -> Name {
        match &p.label {
            Some(l) => self.fresh_named(l),
            None => self.fresh_tmp(),
        }
    }

    /// A labeled call on a plain function: a positional prefix (the pipe
    /// subject) fills the first parameters, labels fill the rest by name
    /// (read off the callee's resolved type), and unfilled parameters
    /// make it a partial application. Evaluation is callee first, then
    /// arguments in WRITTEN order; the saturated in-order call keeps its
    /// direct emission.
    fn labeled_app(
        &mut self,
        f: &Expr<Name>,
        pos_args: &[&Expr<Name>],
        written: &[(&String, &Expr<Name>)],
    ) -> Core {
        let Some(Type::Fun(ps, _)) = self.expr_types.get(&f.span()).cloned() else {
            maxim_panic!("lower_to_core: labeled call without a resolved function type")
        };
        let remaining: Vec<&TParam> = ps[pos_args.len().min(ps.len())..]
            .iter()
            .filter(|p| match &p.label {
                Some(l) => !written.iter().any(|(w, _)| *w == l),
                None => true,
            })
            .collect();
        let declared_labels: Vec<&String> = ps.iter().filter_map(|p| p.label.as_ref()).collect();
        if pos_args.is_empty()
            && remaining.is_empty()
            && written.iter().map(|(l, _)| *l).collect::<Vec<_>>() == declared_labels
        {
            let f2 = self.expr(f);
            let args2: Vec<Core> = written.iter().map(|(_, e)| self.expr(e)).collect();
            Core::App(Box::new(f2), args2)
        } else if pos_args.is_empty() && remaining.is_empty() {
            let f2 = self.expr(f);
            let (wrap_f, fv) = self.hoist(f2);
            let tmps: Vec<(&String, Name, Core)> = written
                .iter()
                .map(|(lbl, e)| {
                    let t = self.fresh_tmp();
                    let c = self.expr(e);
                    (*lbl, t, c)
                })
                .collect();
            let by_label: BTreeMap<&String, &Name> =
                tmps.iter().map(|(lbl, t, _)| (*lbl, t)).collect();
            let mut body = Core::App(
                Box::new(fv),
                ps.iter()
                    .filter_map(|p| p.label.as_ref())
                    .map(|l| Core::Var((*by_label[&l]).clone()))
                    .collect(),
            );
            for (_, t, e) in tmps.into_iter().rev() {
                body = Core::Let(t, Box::new(e), Box::new(body));
            }
            apply_wraps(vec![wrap_f], body)
        } else {
            let f2 = self.expr(f);
            let (wrap_f, fv) = self.hoist(f2);
            let pos_pairs: Vec<(Wrap, Core)> = pos_args
                .iter()
                .map(|e| {
                    let c = self.expr(e);
                    self.hoist(c)
                })
                .collect();
            let lab_pairs: Vec<(&String, (Wrap, Core))> = written
                .iter()
                .map(|(lbl, e)| {
                    let c = self.expr(e);
                    (*lbl, self.hoist(c))
                })
                .collect();
            let params: Vec<Name> = remaining.iter().map(|p| self.param_name(p)).collect();
            let rest_ps = &ps[pos_args.len().min(ps.len())..];
            let by_label: BTreeMap<&String, &Core> = lab_pairs
                .iter()
                .map(|(lbl, (_, v))| (*lbl, v))
                .collect();
            let mut param_queue = params.iter();
            let fill: Vec<Core> = rest_ps
                .iter()
                .map(|p| {
                    match p.label.as_ref().and_then(|l| by_label.get(&l)) {
                        Some(v) => (*v).clone(),
                        None => match param_queue.next() {
                            Some(q) => Core::Var(q.clone()),
                            None => {
                                maxim_panic!("lower_to_core: labeled call ran out of parameters")
                            }
                        },
                    }
                })
                .collect();
            let mut app_args: Vec<Core> =
                pos_pairs.iter().map(|(_, v)| v.clone()).collect();
            app_args.extend(fill);
            let inner = Core::App(Box::new(fv), app_args);
            let body = if params.is_empty() {
                inner
            } else {
                Core::Lam(params.into_iter().map(Some).collect(), Box::new(inner))
            };
            let mut wraps: Vec<Wrap> = pos_pairs.into_iter().map(|(w, _)| w).collect();
            wraps.extend(lab_pairs.into_iter().map(|(_, (w, _))| w));
            apply_wraps(vec![wrap_f], apply_wraps(wraps, body))
        }
    }

    /// Construction. Labeled args reorder to field order (temps preserve
    /// written evaluation order); a positional prefix is a pipe subject;
    /// unfilled fields (or a short positional call) build a PARTIAL
    /// constructor — a closure whose params carry the field names.
    fn construct(&mut self, n: &Name, args: &[Arg<Name>]) -> Core {
        let Some(ci) = self.env.ctors.get(&n.unique).cloned() else {
            maxim_panic!("lower_to_core: unknown constructor after checking")
        };
        let arity = ci.args.len();
        let flds: Vec<String> = ci.fields.clone().unwrap_or_default();
        let pos_args: Vec<&Expr<Name>> = args
            .iter()
            .filter_map(|a| match a {
                Arg::Pos(e) => Some(e),
                Arg::Lab(_, _, _) => None,
            })
            .collect();
        let written: Vec<(&String, &Expr<Name>)> = args
            .iter()
            .filter_map(|a| match a {
                Arg::Lab(_, lbl, e) => Some((lbl, e)),
                Arg::Pos(_) => None,
            })
            .collect();
        let rest_flds: Vec<&String> = flds.iter().skip(pos_args.len()).collect();
        let remaining: Vec<&String> = rest_flds
            .iter()
            .filter(|fld| !written.iter().any(|(l, _)| l == *fld))
            .copied()
            .collect();

        if written.is_empty() && args.len() == arity {
            let args2: Vec<Core> = args.iter().map(|a| self.expr(a.expr())).collect();
            Core::Con(n.clone(), args2)
        } else if pos_args.is_empty()
            && !flds.is_empty()
            && written.iter().map(|(l, _)| (*l).clone()).collect::<Vec<_>>() == flds
        {
            let args2: Vec<Core> = written.iter().map(|(_, e)| self.expr(e)).collect();
            Core::Con(n.clone(), args2)
        } else if pos_args.is_empty() && remaining.is_empty() && !written.is_empty() {
            // written order evaluation, field order construction
            let tmps: Vec<(&String, Name, Core)> = written
                .iter()
                .map(|(lbl, e)| {
                    let t = self.fresh_tmp();
                    let c = self.expr(e);
                    (*lbl, t, c)
                })
                .collect();
            let by_label: BTreeMap<&String, &Name> =
                tmps.iter().map(|(lbl, t, _)| (*lbl, t)).collect();
            let mut body = Core::Con(
                n.clone(),
                flds.iter()
                    .map(|fld| Core::Var((*by_label[&fld]).clone()))
                    .collect(),
            );
            for (_, t, e) in tmps.into_iter().rev() {
                body = Core::Let(t, Box::new(e), Box::new(body));
            }
            body
        } else if !written.is_empty() {
            // pipe subject and/or partial construction
            let pos_pairs: Vec<(Wrap, Core)> = pos_args
                .iter()
                .map(|e| {
                    let c = self.expr(e);
                    self.hoist(c)
                })
                .collect();
            let lab_pairs: Vec<(&String, (Wrap, Core))> = written
                .iter()
                .map(|(lbl, e)| {
                    let c = self.expr(e);
                    (*lbl, self.hoist(c))
                })
                .collect();
            let params: Vec<Name> = remaining.iter().map(|r| self.fresh_named(r)).collect();
            let by_label: BTreeMap<&String, &Core> = lab_pairs
                .iter()
                .map(|(lbl, (_, v))| (*lbl, v))
                .collect();
            let param_for: BTreeMap<&String, &Name> =
                remaining.iter().zip(&params).map(|(r, p)| (*r, p)).collect();
            let fill = |fld: &String| -> Core {
                match by_label.get(&fld) {
                    Some(v) => (*v).clone(),
                    None => Core::Var((*param_for[&fld]).clone()),
                }
            };
            let mut con_args: Vec<Core> = pos_pairs.iter().map(|(_, v)| v.clone()).collect();
            con_args.extend(rest_flds.iter().map(|fld| fill(fld)));
            let inner = Core::Con(n.clone(), con_args);
            let body = if params.is_empty() {
                inner
            } else {
                Core::Lam(params.into_iter().map(Some).collect(), Box::new(inner))
            };
            let mut wraps: Vec<Wrap> = pos_pairs.into_iter().map(|(w, _)| w).collect();
            wraps.extend(lab_pairs.into_iter().map(|(_, (w, _))| w));
            apply_wraps(wraps, body)
        } else {
            // positional partial construction
            let pairs: Vec<(Wrap, Core)> = args
                .iter()
                .map(|a| {
                    let c = self.expr(a.expr());
                    self.hoist(c)
                })
                .collect();
            let ls: Vec<Option<&String>> = match &ci.fields {
                Some(fs) => fs.iter().map(Some).collect(),
                None => vec![None; arity],
            };
            let params: Vec<Name> = ls[args.len()..]
                .iter()
                .map(|ml| match ml {
                    Some(l) => self.fresh_named(l),
                    None => self.fresh_tmp(),
                })
                .collect();
            let mut con_args: Vec<Core> = pairs.iter().map(|(_, v)| v.clone()).collect();
            con_args.extend(params.iter().map(|p| Core::Var(p.clone())));
            let lam = Core::Lam(
                params.into_iter().map(Some).collect(),
                Box::new(Core::Con(n.clone(), con_args)),
            );
            apply_wraps(pairs.into_iter().map(|(w, _)| w).collect(), lam)
        }
    }

    fn eta_con(&mut self, n: &Name) -> Core {
        let Some(ci) = self.env.ctors.get(&n.unique).cloned() else {
            maxim_panic!("lower_to_core: unknown constructor after checking")
        };
        match ci.args.len() {
            0 => Core::Con(n.clone(), Vec::new()),
            k => {
                // Record fields name the eta params: `(name, age) => ...`.
                let ps: Vec<Name> = match &ci.fields {
                    Some(fs) => fs.iter().map(|f| self.fresh_named(f)).collect(),
                    None => (0..k).map(|_| self.fresh_tmp()).collect(),
                };
                Core::Lam(
                    ps.iter().cloned().map(Some).collect(),
                    Box::new(Core::Con(
                        n.clone(),
                        ps.into_iter().map(Core::Var).collect(),
                    )),
                )
            }
        }
    }

    fn eta_intrinsic(&mut self, i: Intrinsic) -> Core {
        let ps: Vec<Name> = (0..i.arity()).map(|_| self.fresh_tmp()).collect();
        Core::Lam(
            ps.iter().cloned().map(Some).collect(),
            Box::new(Core::Intrinsic(
                i,
                ps.into_iter().map(Core::Var).collect(),
            )),
        )
    }
}

/// JSX to the AST it sugars: elements become `Html.node` calls,
/// component tags become saturated labeled `view` calls (the checker
/// recorded the callee's type at the tag span, so `labeled_app` finds
/// it), adjacent text runs and `${}` holes coalesce into ONE `Html.text`
/// of an interpolation, and `{..}` spreads stitch with `List.append` —
/// a trailing spread rides the list literal's own tail.
fn desugar_jsx(
    api: &JsxApi,
    instances: &BTreeMap<Span, InstLower>,
    el: &JsxElem<Name>,
) -> Expr<Name> {
    match &el.tag {
        JsxTag::Intrinsic(tsp, tag) => Expr::Call(
            el.span,
            Box::new(Expr::Var(*tsp, api.node.clone())),
            vec![
                Arg::Pos(Expr::Lit(*tsp, Lit::Str(tag.clone()))),
                Arg::Pos(Expr::List(
                    *tsp,
                    el.attrs.iter().map(|a| attr_expr(api, a)).collect(),
                    None,
                )),
                Arg::Pos(children_expr(api, instances, el.span, &el.children)),
            ],
        ),
        JsxTag::Component(tsp, _) => match instances.get(tsp) {
            // Stateless: the saturated labeled call of the wired view.
            Some(InstLower::Call(view_name)) => {
                let mut args: Vec<Arg<Name>> = el
                    .attrs
                    .iter()
                    .filter_map(|a| match a {
                        JsxAttr::Attr(asp, name, Some(v)) => {
                            Some(Arg::Lab(*asp, name.clone(), v.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                if el.has_close {
                    args.push(Arg::Lab(
                        el.span,
                        "children".to_string(),
                        children_expr(api, instances, el.span, &el.children),
                    ));
                }
                Expr::Call(el.span, Box::new(Expr::Var(*tsp, view_name.clone())), args)
            }
            // Stateful: an instance node — Props construction with
            // Some/None fills for optional fields, out handlers by event
            // name.
            Some(InstLower::Node(comp, m_props)) => {
                let given_lbls: Vec<&String> = el
                    .attrs
                    .iter()
                    .filter_map(|a| match a {
                        JsxAttr::Attr(_, name, Some(_)) => Some(name),
                        _ => None,
                    })
                    .collect();
                let props = m_props.as_ref().map(|(ctor_name, optionals)| {
                    let some_wrap = |name: &String, v: &Expr<Name>| -> Expr<Name> {
                        if optionals.contains(name) {
                            Expr::Call(
                                v.span(),
                                Box::new(Expr::Con(v.span(), api.some.clone())),
                                vec![Arg::Pos(v.clone())],
                            )
                        } else {
                            v.clone()
                        }
                    };
                    let mut ctor_args: Vec<Arg<Name>> = el
                        .attrs
                        .iter()
                        .filter_map(|a| match a {
                            JsxAttr::Attr(asp, name, Some(v)) => {
                                Some(Arg::Lab(*asp, name.clone(), some_wrap(name, v)))
                            }
                            _ => None,
                        })
                        .collect();
                    for l in optionals {
                        if !given_lbls.contains(&l) {
                            ctor_args.push(Arg::Lab(
                                *tsp,
                                l.clone(),
                                Expr::Con(*tsp, api.none.clone()),
                            ));
                        }
                    }
                    Box::new(Expr::Call(
                        *tsp,
                        Box::new(Expr::Con(*tsp, ctor_name.clone())),
                        ctor_args,
                    ))
                });
                Expr::Instance(
                    el.span,
                    comp.clone(),
                    props,
                    el.attrs
                        .iter()
                        .filter_map(|a| match a {
                            JsxAttr::On(_, ev, h) => Some((ev.clone(), h.clone())),
                            _ => None,
                        })
                        .collect(),
                )
            }
            None => maxim_panic!("desugar_jsx: component tag with no lowering info"),
        },
    }
}

fn attr_expr(api: &JsxApi, a: &JsxAttr<Name>) -> Expr<Name> {
    match a {
        JsxAttr::Attr(asp, name, v) => Expr::Call(
            *asp,
            Box::new(Expr::Var(*asp, api.attr.clone())),
            vec![
                Arg::Pos(Expr::Lit(*asp, Lit::Str(name.clone()))),
                Arg::Pos(match v {
                    Some(e) => e.clone(),
                    None => Expr::Lit(*asp, Lit::Str(String::new())),
                }),
            ],
        ),
        JsxAttr::On(asp, ev, e) => {
            // The payload table, mirrored from the checker: `input`
            // handlers take the target value, `change` handlers the
            // checkbox state.
            let target = match ev.as_str() {
                "input" => &api.on_text,
                "change" => &api.on_bool,
                _ => &api.on,
            };
            Expr::Call(
                *asp,
                Box::new(Expr::Var(*asp, target.clone())),
                vec![
                    Arg::Pos(Expr::Lit(*asp, Lit::Str(ev.clone()))),
                    Arg::Pos(e.clone()),
                ],
            )
        }
    }
}

fn children_expr(
    api: &JsxApi,
    instances: &BTreeMap<Span, InstLower>,
    sp: Span,
    children: &[JsxChild<Name>],
) -> Expr<Name> {
    let coalesced = coalesce(children);
    go(api, instances, sp, &coalesced)
}

fn go(
    api: &JsxApi,
    instances: &BTreeMap<Span, InstLower>,
    sp: Span,
    cs: &[JsxChild<Name>],
) -> Expr<Name> {
    let single = |c: &JsxChild<Name>| -> Expr<Name> {
        let text_call = |csp: Span, e: Expr<Name>| {
            Expr::Call(
                csp,
                Box::new(Expr::Var(csp, api.text.clone())),
                vec![Arg::Pos(e)],
            )
        };
        match c {
            JsxChild::Text(csp, t) => text_call(*csp, Expr::Lit(*csp, Lit::Str(t.clone()))),
            JsxChild::Hole(csp, e) => text_call(*csp, e.clone()),
            JsxChild::Splice(_, e) => e.clone(),
            JsxChild::Elem(el2) => desugar_jsx(api, instances, el2),
            JsxChild::Spread(_, _) => maxim_panic!("desugar_jsx: spread in a single position"),
        }
    };
    let split = cs
        .iter()
        .position(|c| matches!(c, JsxChild::Spread(_, _)));
    match split {
        None => Expr::List(sp, cs.iter().map(single).collect(), None),
        Some(i) => {
            let before: Vec<Expr<Name>> = cs[..i].iter().map(single).collect();
            let JsxChild::Spread(ssp, e) = &cs[i] else {
                unreachable!()
            };
            let rest = &cs[i + 1..];
            if rest.is_empty() {
                Expr::List(sp, before, Some(Box::new(e.clone())))
            } else {
                let rest_e = go(api, instances, sp, rest);
                let appended = Expr::Call(
                    *ssp,
                    Box::new(Expr::Var(*ssp, api.append.clone())),
                    vec![Arg::Pos(e.clone()), Arg::Pos(rest_e)],
                );
                Expr::List(sp, before, Some(Box::new(appended)))
            }
        }
    }
}

/// `count: ${n}!` is ONE text node: `Html.text("count: " + n + "!")`.
fn coalesce(children: &[JsxChild<Name>]) -> Vec<JsxChild<Name>> {
    fn text_part(c: &JsxChild<Name>) -> Option<InterpPart<Name>> {
        match c {
            JsxChild::Text(_, t) => Some(InterpPart::Str(t.clone())),
            JsxChild::Hole(_, e) => Some(InterpPart::Expr(e.clone())),
            _ => None,
        }
    }
    fn child_span(c: &JsxChild<Name>) -> Span {
        match c {
            JsxChild::Text(sp, _)
            | JsxChild::Hole(sp, _)
            | JsxChild::Splice(sp, _)
            | JsxChild::Spread(sp, _) => *sp,
            JsxChild::Elem(e) => e.span,
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let c = &children[i];
        match text_part(c) {
            None => {
                out.push(c.clone());
                i += 1;
            }
            Some(p0) => {
                let mut parts = vec![p0];
                let mut j = i + 1;
                while j < children.len() {
                    match text_part(&children[j]) {
                        Some(p) => {
                            parts.push(p);
                            j += 1;
                        }
                        None => break,
                    }
                }
                if parts.len() == 1 {
                    out.push(c.clone());
                    i += 1;
                } else {
                    let sp = child_span(c);
                    out.push(JsxChild::Hole(sp, Expr::Interp(sp, parts)));
                    i = j;
                }
            }
        }
    }
    out
}

pub fn intrinsic_of(n: &Name) -> Option<Intrinsic> {
    let i = if *n == b::intr_app_mount() {
        Intrinsic::AppMount
    } else if *n == b::intr_html_resolve() {
        Intrinsic::HtmlResolve
    } else if *n == b::intr_debug_log() {
        Intrinsic::DebugLog
    } else if *n == b::intr_string_length() {
        Intrinsic::StrLength
    } else if *n == b::intr_string_replace() {
        Intrinsic::StrReplace
    } else if *n == b::intr_int_to_string() {
        Intrinsic::IntToString
    } else if *n == b::intr_float_to_string() {
        Intrinsic::FloatToString
    } else if *n == b::intr_float_from_int() {
        Intrinsic::FloatFromInt
    } else if *n == b::intr_int_from_float() {
        Intrinsic::IntFromFloat
    } else if *n == b::intr_float_trunc() {
        Intrinsic::FloatTrunc
    } else if *n == b::intr_float_floor() {
        Intrinsic::FloatFloor
    } else if *n == b::intr_float_ceil() {
        Intrinsic::FloatCeil
    } else if *n == b::intr_float_round() {
        Intrinsic::FloatRound
    } else {
        return None;
    };
    Some(i)
}
