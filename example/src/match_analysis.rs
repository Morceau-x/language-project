//! Pattern-match analysis, straight from Maranget: the pattern matrix
//! with specialize/default powers both the usefulness checker
//! (exhaustiveness witnesses like `Rect(_, _)` and redundancy warnings)
//! and compilation to decision trees (which map onto readable nested
//! `switch` output, with arm bodies shared via join points downstream).

use crate::ast::{Arm, Lit, Module, Pat, PatRest};
use crate::builtin::{ctor_cons, ctor_nil};
use crate::diag::Diagnostic;
use crate::names::Name;
use crate::span::Span;
use crate::types::env::TcEnv;

/// A path from the scrutinee root into a value: `Arg(i, c, p)` is the
/// i-th (0-based) argument of the constructor `c` at `p`. Carrying the
/// constructor lets the backend pick the right JS accessor (`._0` for
/// ADT slots, `.label` for record fields).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Occ {
    Root,
    Arg(usize, Name, Box<Occ>),
}

#[derive(Clone, Debug)]
pub enum DTree {
    /// Run this arm (0-based index) with these pattern bindings.
    Leaf(usize, Vec<(Name, Occ)>),
    /// The arm's pattern matched but it carries a guard: bind, test,
    /// and fall back to the rest of the matrix when the guard is false.
    Guard(usize, Vec<(Name, Occ)>, Box<DTree>),
    /// Branch on the constructor tag at an occurrence. The default is
    /// `None` exactly when the listed constructors are a complete
    /// signature.
    SwitchCtor(Occ, Vec<(Name, usize, DTree)>, Option<Box<DTree>>),
    SwitchLit(Occ, Vec<(Lit, DTree)>, Box<DTree>),
    /// Unreachable when the match is exhaustive; emitted as a defensive
    /// throw.
    Fail,
}

/// Simplified patterns: variables erased to wildcards (bindings are
/// collected separately as occurrence paths), list sugar expanded to
/// Cons/Nil so the matrix only ever sees constructors and literals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SP {
    Wild,
    Ctor(Name, usize, Vec<SP>),
    Lit(Lit),
}

fn simplify(pat: &Pat<Name>) -> SP {
    // [p, q] => Cons(p, Cons(q, Nil)); [p, ..r] => Cons(p, _).
    fn desugar_list(elems: &[Pat<Name>], rest: &Option<PatRest<Name>>) -> SP {
        match elems {
            [] => match rest {
                Some(_) => SP::Wild,
                None => SP::Ctor(ctor_nil(), 0, Vec::new()),
            },
            [p, ps @ ..] => SP::Ctor(ctor_cons(), 2, vec![simplify(p), desugar_list(ps, rest)]),
        }
    }
    match pat {
        Pat::Wild(_) | Pat::Var(_, _) => SP::Wild,
        Pat::Lit(_, l) => SP::Lit(l.clone()),
        Pat::Ctor(_, n, args) => SP::Ctor(n.clone(), args.len(), args.iter().map(simplify).collect()),
        Pat::List(_, elems, rest) => desugar_list(elems, rest),
    }
}

/// Bindings of a pattern as occurrence paths from the scrutinee.
fn collect_binds(pat: &Pat<Name>, occ: &Occ, out: &mut Vec<(Name, Occ)>) {
    fn go_list(
        elems: &[Pat<Name>],
        rest: &Option<PatRest<Name>>,
        o: &Occ,
        out: &mut Vec<(Name, Occ)>,
    ) {
        let [p, ps @ ..] = elems else { return };
        collect_binds(p, &Occ::Arg(0, ctor_cons(), Box::new(o.clone())), out);
        let tail = Occ::Arg(1, ctor_cons(), Box::new(o.clone()));
        if ps.is_empty() {
            if let Some(PatRest { name: Some(n), .. }) = rest {
                out.push((n.clone(), tail));
            }
        } else {
            go_list(ps, rest, &tail, out);
        }
    }
    match pat {
        Pat::Wild(_) | Pat::Lit(_, _) => {}
        Pat::Var(_, n) => out.push((n.clone(), occ.clone())),
        Pat::Ctor(_, c, args) => {
            for (i, p) in args.iter().enumerate() {
                collect_binds(p, &Occ::Arg(i, c.clone(), Box::new(occ.clone())), out);
            }
        }
        Pat::List(_, elems, rest) => go_list(elems, rest, occ, out),
    }
}

/// The complete constructor set (with arities) behind a constructor, via
/// its type. Builtin Bool and List are in the base env like everything
/// else.
pub type Siblings<'a> = &'a dyn Fn(&Name) -> Option<Vec<(Name, usize)>>;

fn siblings_in(env: &TcEnv) -> impl Fn(&Name) -> Option<Vec<(Name, usize)>> + '_ {
    move |ctor: &Name| {
        let ci = env.ctors.get(&ctor.unique)?;
        let ti = env.ty_cons.get(&ci.ty_con.name.unique)?;
        Some(
            ti.ctors
                .iter()
                .map(|c| (c.name.clone(), c.args.len()))
                .collect(),
        )
    }
}

// Matrix operations -------------------------------------------------------------

type Row = (Vec<SP>, usize);

fn specialize(c: &Name, arity: usize, rows: &[Row]) -> Vec<Row> {
    rows.iter()
        .filter_map(|(ps, i)| match ps.split_first() {
            Some((SP::Ctor(c2, _, args), rest)) if c2 == c => {
                let mut v = args.clone();
                v.extend(rest.iter().cloned());
                Some((v, *i))
            }
            Some((SP::Wild, rest)) => {
                let mut v = vec![SP::Wild; arity];
                v.extend(rest.iter().cloned());
                Some((v, *i))
            }
            _ => None,
        })
        .collect()
}

fn specialize_lit(l: &Lit, rows: &[Row]) -> Vec<Row> {
    rows.iter()
        .filter_map(|(ps, i)| match ps.split_first() {
            Some((SP::Lit(l2), rest)) if l2 == l => Some((rest.to_vec(), *i)),
            Some((SP::Wild, rest)) => Some((rest.to_vec(), *i)),
            _ => None,
        })
        .collect()
}

fn default_matrix(rows: &[Row]) -> Vec<Row> {
    rows.iter()
        .filter_map(|(ps, i)| match ps.split_first() {
            Some((SP::Wild, rest)) => Some((rest.to_vec(), *i)),
            _ => None,
        })
        .collect()
}

fn head_ctors(rows: &[Row]) -> Vec<(Name, usize)> {
    let mut out: Vec<(Name, usize)> = Vec::new();
    for (ps, _) in rows {
        if let Some(SP::Ctor(c, a, _)) = ps.first() {
            if !out.iter().any(|(c2, a2)| c2 == c && a2 == a) {
                out.push((c.clone(), *a));
            }
        }
    }
    out
}

fn head_lits(rows: &[Row]) -> Vec<Lit> {
    let mut out: Vec<Lit> = Vec::new();
    for (ps, _) in rows {
        if let Some(SP::Lit(l)) = ps.first() {
            if !out.contains(l) {
                out.push(l.clone());
            }
        }
    }
    out
}

/// Is the set of head constructors a complete signature?
fn complete_sig(siblings: Siblings, rows: &[Row]) -> Option<Vec<(Name, usize)>> {
    let hs = head_ctors(rows);
    let (c, _) = hs.first()?;
    let sibs = siblings(c)?;
    if sibs.iter().all(|(s, _)| hs.iter().any(|(h, _)| h == s)) {
        Some(sibs)
    } else {
        None
    }
}

/// Maranget usefulness: is a row of wildcards (width n) useful w.r.t.
/// the matrix — i.e. is something NOT covered? Returns a witness vector.
fn missing(siblings: Siblings, rows: &[Row], n: usize) -> Option<Vec<SP>> {
    if n == 0 {
        return if rows.is_empty() { Some(Vec::new()) } else { None };
    }
    match complete_sig(siblings, rows) {
        Some(sigs) => sigs.iter().find_map(|(c, a)| {
            let sub = missing(siblings, &specialize(c, *a, rows), a + n - 1)?;
            let (args, rest) = sub.split_at(*a);
            let mut w = vec![SP::Ctor(c.clone(), *a, args.to_vec())];
            w.extend(rest.iter().cloned());
            Some(w)
        }),
        None => {
            let rest = missing(siblings, &default_matrix(rows), n - 1)?;
            let hs = head_ctors(rows);
            let witness_head = if hs.is_empty() {
                SP::Wild
            } else {
                match unused_sibling(siblings, &hs) {
                    Some((c, a)) => SP::Ctor(c, a, vec![SP::Wild; a]),
                    None => SP::Wild,
                }
            };
            let mut w = vec![witness_head];
            w.extend(rest);
            Some(w)
        }
    }
}

fn unused_sibling(siblings: Siblings, hs: &[(Name, usize)]) -> Option<(Name, usize)> {
    let (c0, _) = hs.first()?;
    let sibs = siblings(c0)?;
    sibs.into_iter()
        .find(|(s, _)| !hs.iter().any(|(h, _)| h == s))
}

/// Is row `v` useful w.r.t. the rows above it? (Redundancy check.)
fn useful(siblings: Siblings, rows: &[Row], v: &[SP]) -> bool {
    if rows.is_empty() {
        return true;
    }
    let Some((p, ps)) = v.split_first() else {
        return rows.is_empty();
    };
    match p {
        SP::Ctor(c, a, args) => {
            let mut sub = args.clone();
            sub.extend(ps.iter().cloned());
            useful(siblings, &specialize(c, *a, rows), &sub)
        }
        SP::Lit(l) => useful(siblings, &specialize_lit(l, rows), ps),
        SP::Wild => match complete_sig(siblings, rows) {
            Some(sigs) => sigs.iter().any(|(c, a)| {
                let mut sub = vec![SP::Wild; *a];
                sub.extend(ps.iter().cloned());
                useful(siblings, &specialize(c, *a, rows), &sub)
            }),
            None => useful(siblings, &default_matrix(rows), ps),
        },
    }
}

// Decision-tree compilation -------------------------------------------------------

pub fn compile_tree(
    siblings: Siblings,
    guarded: &dyn Fn(usize) -> bool,
    occs: &[Occ],
    rows: &[(Vec<SP>, usize, Vec<(Name, Occ)>)],
) -> DTree {
    let Some((ps0, arm0, binds0)) = rows.first() else {
        return DTree::Fail;
    };
    let is_wild = |p: &SP| matches!(p, SP::Wild);
    if ps0.iter().all(is_wild) {
        return if guarded(*arm0) {
            DTree::Guard(
                *arm0,
                binds0.clone(),
                Box::new(compile_tree(siblings, guarded, occs, &rows[1..])),
            )
        } else {
            DTree::Leaf(*arm0, binds0.clone())
        };
    }
    let j = ps0.iter().position(|p| !is_wild(p)).unwrap_or(0);
    fn swap<T: Clone>(k: usize, xs: &[T]) -> Vec<T> {
        let mut out = Vec::with_capacity(xs.len());
        out.push(xs[k].clone());
        out.extend(xs[..k].iter().cloned());
        out.extend(xs[k + 1..].iter().cloned());
        out
    }
    let rows2: Vec<(Vec<SP>, usize, Vec<(Name, Occ)>)> = rows
        .iter()
        .map(|(ps, i, b)| (swap(j, ps), *i, b.clone()))
        .collect();
    let plain_rows: Vec<Row> = rows2.iter().map(|(ps, i, _)| (ps.clone(), *i)).collect();
    let binds_of = |i: usize| -> Vec<(Name, Occ)> {
        rows2
            .iter()
            .find(|(_, i2, _)| *i2 == i)
            .map(|(_, _, b)| b.clone())
            .unwrap_or_default()
    };
    let rebind = |sub: Vec<Row>| -> Vec<(Vec<SP>, usize, Vec<(Name, Occ)>)> {
        sub.into_iter().map(|(ps, i)| (ps, i, binds_of(i))).collect()
    };
    let swapped_occs = swap(j, occs);
    let Some((occ_j, occ_rest)) = swapped_occs.split_first() else {
        return DTree::Fail; // unreachable: occs and ps0 have the same width
    };
    let hs = head_ctors(&plain_rows);
    if hs.is_empty() {
        // Literal column.
        let lits = head_lits(&plain_rows);
        let branches: Vec<(Lit, DTree)> = lits
            .into_iter()
            .map(|l| {
                let t = compile_tree(
                    siblings,
                    guarded,
                    occ_rest,
                    &rebind(specialize_lit(&l, &plain_rows)),
                );
                (l, t)
            })
            .collect();
        let deflt = compile_tree(
            siblings,
            guarded,
            occ_rest,
            &rebind(default_matrix(&plain_rows)),
        );
        DTree::SwitchLit(occ_j.clone(), branches, Box::new(deflt))
    } else {
        let complete = complete_sig(siblings, &plain_rows).is_some();
        let branches: Vec<(Name, usize, DTree)> = hs
            .iter()
            .map(|(c, a)| {
                let mut sub_occs: Vec<Occ> = (0..*a)
                    .map(|i| Occ::Arg(i, c.clone(), Box::new(occ_j.clone())))
                    .collect();
                sub_occs.extend(occ_rest.iter().cloned());
                let t = compile_tree(
                    siblings,
                    guarded,
                    &sub_occs,
                    &rebind(specialize(c, *a, &plain_rows)),
                );
                (c.clone(), *a, t)
            })
            .collect();
        let deflt = if complete {
            None
        } else {
            Some(Box::new(compile_tree(
                siblings,
                guarded,
                occ_rest,
                &rebind(default_matrix(&plain_rows)),
            )))
        };
        DTree::SwitchCtor(occ_j.clone(), branches, deflt)
    }
}

// Entry points ----------------------------------------------------------------------

/// Analyze one `match`: exhaustiveness (E4001 with a witness), per-arm
/// redundancy (W4002), and the compiled decision tree.
pub fn analyze_match(
    env: &TcEnv,
    match_span: Span,
    arms: &[Arm<Name>],
) -> (Vec<Diagnostic>, DTree) {
    let sib_fn = siblings_in(env);
    let siblings: Siblings = &sib_fn;
    let pats: Vec<SP> = arms.iter().map(|a| simplify(&a.pat)).collect();
    let is_guarded = |i: usize| arms[i].guard.is_some();

    // A guarded arm may fail at runtime, so only unguarded rows count as
    // coverage — for exhaustiveness and for shadowing later arms alike.
    let covering_rows_before = |i: usize| -> Vec<Row> {
        pats.iter()
            .enumerate()
            .filter(|(j, _)| *j < i && !is_guarded(*j))
            .map(|(j, p)| (vec![p.clone()], j))
            .collect()
    };
    let covering_rows: Vec<Row> = pats
        .iter()
        .enumerate()
        .filter(|(j, _)| !is_guarded(*j))
        .map(|(j, p)| (vec![p.clone()], j))
        .collect();

    let mut diags: Vec<Diagnostic> = Vec::new();
    if let Some(witness) = missing(siblings, &covering_rows, 1) {
        let mut hints = vec!["Add the missing pattern, or a final `_ ->` arm.".to_string()];
        if (0..arms.len()).any(is_guarded) {
            hints.push(
                "A guarded arm can fail its guard, so it never counts as coverage.".to_string(),
            );
        }
        diags.push(
            Diagnostic::error(
                "E4001",
                "MISSING PATTERNS",
                match_span,
                format!("this `match` does not cover {}", render_witness(&witness)),
            )
            .with_hints(hints),
        );
    }
    for (i, arm) in arms.iter().enumerate() {
        if !useful(siblings, &covering_rows_before(i), &[pats[i].clone()]) {
            diags.push(Diagnostic::warning(
                "W4002",
                "REDUNDANT PATTERN",
                arm.span,
                "this pattern can never match",
            ));
        }
    }

    let rows: Vec<(Vec<SP>, usize, Vec<(Name, Occ)>)> = arms
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let mut binds = Vec::new();
            collect_binds(&a.pat, &Occ::Root, &mut binds);
            (vec![pats[i].clone()], i, binds)
        })
        .collect();
    let tree = compile_tree(siblings, &is_guarded, &[Occ::Root], &rows);
    (diags, tree)
}

fn render_witness(sps: &[SP]) -> String {
    sps.iter().map(render_sp).collect::<Vec<_>>().join(", ")
}

fn render_sp(sp: &SP) -> String {
    match sp {
        SP::Wild => "_".to_string(),
        SP::Lit(Lit::Int(n)) => n.to_string(),
        SP::Lit(Lit::Float(d)) => d.show(),
        SP::Lit(Lit::Str(s)) => format!("{s:?}"),
        SP::Ctor(c, _, args) => {
            if *c == ctor_nil() || *c == ctor_cons() {
                format!("[{}]", render_list(sp))
            } else if args.is_empty() {
                c.text.clone()
            } else {
                format!(
                    "{}({})",
                    c.text,
                    args.iter().map(render_sp).collect::<Vec<_>>().join(", ")
                )
            }
        }
    }
}

fn render_list(sp: &SP) -> String {
    match sp {
        SP::Ctor(c, _, args) if *c == ctor_nil() && args.is_empty() => String::new(),
        SP::Ctor(c, _, args) if *c == ctor_cons() && args.len() == 2 => {
            let (h, t) = (&args[0], &args[1]);
            match t {
                SP::Ctor(c2, _, targs) if *c2 == ctor_nil() && targs.is_empty() => render_sp(h),
                SP::Ctor(c2, _, _) if *c2 == ctor_cons() => {
                    format!("{}, {}", render_sp(h), render_list(t))
                }
                _ => format!("{}, ..", render_sp(h)),
            }
        }
        other => render_sp(other),
    }
}

/// Every `match` in a module body, with its span and arms (nested ones
/// included) — the work-list for analysis and lowering.
pub fn collect_matches<'a>(m: &'a Module<Name>) -> Vec<(Span, &'a [Arm<Name>])> {
    use crate::ast::*;
    fn go<'a>(e: &'a Expr<Name>, out: &mut Vec<(Span, &'a [Arm<Name>])>) {
        match e {
            Expr::Match(sp, scrut, arms) => {
                out.push((*sp, arms));
                go(scrut, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        go(g, out);
                    }
                    go(&arm.body, out);
                }
            }
            // EInstance is synthesized after this pass; the arm is for
            // totality.
            Expr::Instance(_, _, mp, outs) => {
                if let Some(p) = mp {
                    go(p, out);
                }
                for (_, e2) in outs {
                    go(e2, out);
                }
            }
            Expr::Var(_, _) | Expr::Con(_, _) | Expr::Lit(_, _) => {}
            Expr::Dot(_, e2, _, _) | Expr::Field(_, e2, _, _) => go(e2, out),
            Expr::Call(_, f, args) => {
                go(f, out);
                for a in args {
                    go(a.expr(), out);
                }
            }
            Expr::Update(_, _, _, base, fields) => {
                go(base, out);
                for (_, _, e2) in fields {
                    go(e2, out);
                }
            }
            Expr::Bin(_, _, _, l, r) | Expr::Pipe(_, _, l, r) => {
                go(l, out);
                go(r, out);
            }
            Expr::Unary(_, _, e2) | Expr::Try(_, _, e2) | Expr::Cast(_, e2, _, _) => go(e2, out),
            Expr::If(_, c, t, e2) => {
                go(c, out);
                go(t, out);
                go(e2, out);
            }
            Expr::Lam(_, _, _, body) => go(body, out),
            Expr::List(_, es, spread) => {
                for e2 in es {
                    go(e2, out);
                }
                if let Some(s) = spread {
                    go(s, out);
                }
            }
            Expr::Block(_, stmts, result) => {
                for s in stmts {
                    match s {
                        Stmt::Let(_, _, _, e2) | Stmt::Discard(e2) => go(e2, out),
                    }
                }
                go(result, out);
            }
            Expr::Interp(_, parts) => {
                for p in parts {
                    if let InterpPart::Expr(e2) = p {
                        go(e2, out);
                    }
                }
            }
            Expr::Jsx(_, el) => jsx_go(el, out),
        }
    }
    fn jsx_go<'a>(el: &'a JsxElem<Name>, out: &mut Vec<(Span, &'a [Arm<Name>])>) {
        for a in &el.attrs {
            match a {
                JsxAttr::Attr(_, _, v) => {
                    if let Some(e) = v {
                        go(e, out);
                    }
                }
                JsxAttr::On(_, _, e) => go(e, out),
            }
        }
        for c in &el.children {
            match c {
                JsxChild::Text(_, _) => {}
                JsxChild::Elem(el2) => jsx_go(el2, out),
                JsxChild::Splice(_, e) | JsxChild::Spread(_, e) | JsxChild::Hole(_, e) => {
                    go(e, out)
                }
            }
        }
    }
    let mut out = Vec::new();
    for d in &m.decls {
        match d {
            Decl::Type(_) | Decl::Component(_) => {}
            Decl::Fn(fnd) => go(&fnd.body, &mut out),
            Decl::Let(_, _, _, e) => go(e, &mut out),
            Decl::Cast(cd) => go(&cd.body, &mut out),
        }
    }
    out
}

#[cfg(test)]
mod tests;
