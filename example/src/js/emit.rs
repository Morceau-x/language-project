//! Core to JavaScript via destination-passing: every expression is
//! emitted toward a destination (return position, an assignment target,
//! or a value), so value-position ifs and matches become a `let` plus
//! statements — never an IIFE. Decision trees become `switch (x.$)` with
//! arm bodies shared as join-point arrows when a tree duplicates a leaf.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{BinOp, Lit, TryKind, UnOp};
use crate::builtin as b;
use crate::core::{Core, CoreDecl, CoreModule, Intrinsic, core_refs};
use crate::js::syntax::*;
use crate::match_analysis::{DTree, Occ};
use crate::maxim_panic;
use crate::names::{ModuleName, Name, NameSort};
use crate::types::env::{CtorInfo, Iface, TcEnv};
use crate::unique::Unique;

/// One JS block scope: the names declared in this block (taken), and the
/// enclosing-scope names this block's code references (blocked). A fresh
/// binding may not reuse a blocked name — `const` covers its whole block,
/// so the enclosing reference would become a temporal-dead-zone crash (or
/// a silent rebind) instead of a shadow.
#[derive(Clone, Debug)]
struct Frame {
    taken: BTreeSet<String>,
    blocked: BTreeSet<String>,
}

struct Es {
    scope: BTreeMap<Unique, String>,
    /// Innermost JS block LAST; the base frame is module scope. Never
    /// empty. Frames track only source-derived names: `$`-prefixed temps
    /// and join points are disjoint by construction (Maxim identifiers
    /// cannot contain `$` and `mangle` only appends one).
    frames: Vec<Frame>,
    imports: BTreeSet<ModuleName>,
    runtime: bool,
    tmp: i64,
}

struct Env<'a> {
    tc: &'a TcEnv,
    module: &'a ModuleName,
}

/// Emit one module as ES-module source. The `Iface` is the module's own
/// (drives nullary-constructor consts).
pub fn emit_module(tc_env: &TcEnv, own_iface: &Iface, cm: &CoreModule) -> String {
    let env = Env {
        tc: tc_env,
        module: &cm.name,
    };
    // The module frame owns every module-scope JS name up front:
    // top-level declarations and nullary-ctor consts. A CoreLet's init
    // statements execute at module scope (ExportLetInit), so their locals
    // allocate from this frame and must dodge all of these. Import
    // aliases and $rt are capitalized / $-prefixed — unreachable by value
    // names — and need no seeding.
    let nullary_ctors: Vec<&CtorInfo> = own_iface
        .ty_cons
        .values()
        .flat_map(|ti| ti.ctors.iter())
        .filter(|ci| ci.args.is_empty())
        .collect();
    let mut module_taken: BTreeSet<String> = nullary_ctors
        .iter()
        .map(|ci| mangle(&ci.name.text))
        .collect();
    for d in &cm.decls {
        let n = match d {
            CoreDecl::Fn(n, _, _) | CoreDecl::Let(n, _) | CoreDecl::Component(n, _, _) => n,
        };
        module_taken.insert(mangle(&n.text));
    }
    let mut es = Es {
        scope: BTreeMap::new(),
        frames: vec![Frame {
            taken: module_taken,
            blocked: BTreeSet::new(),
        }],
        imports: BTreeSet::new(),
        runtime: false,
        tmp: 0,
    };
    let ctor_consts: Vec<JsDecl> = nullary_ctors
        .iter()
        .map(|ci| {
            JsDecl::ExportConst(
                mangle(&ci.name.text),
                JsExpr::Obj(vec![("$".to_string(), JsExpr::Str(ci.name.text.clone()))]),
            )
        })
        .collect();
    let decls_js: Vec<JsDecl> = cm
        .decls
        .iter()
        .map(|d| emit_decl(&env, &mut es, d))
        .collect();
    let mut imports: Vec<(String, String)> = es
        .imports
        .iter()
        .map(|m| (m.0.clone(), format!("./{}.mjs", m.0)))
        .collect();
    if es.runtime {
        imports.push(("$rt".to_string(), "./maxim_runtime.mjs".to_string()));
    }
    let mut decls = ctor_consts;
    decls.extend(decls_js);
    render_js_module(&JsModule { imports, decls })
}

fn emit_decl(env: &Env, es: &mut Es, d: &CoreDecl) -> JsDecl {
    match d {
        CoreDecl::Fn(n, params, body) => in_block(env, es, &[body], |env, es| {
            let ps: Vec<String> = params.iter().map(|p| bind_param(es, p)).collect();
            let stmts = emit_to(env, es, &Dest::Ret, body);
            JsDecl::ExportFn(mangle(&n.text), ps, stmts)
        }),
        CoreDecl::Let(n, body) => {
            // Init statements run at module scope (ExportLetInit renders
            // them unindented), so locals allocate from the seeded module
            // frame — no scope push here.
            let (ss, e) = to_expr(env, es, body);
            let name = mangle(&n.text);
            if ss.is_empty() {
                JsDecl::ExportConst(name, e)
            } else {
                let mut body_stmts = ss;
                body_stmts.push(JsStmt::Assign(name.clone(), e));
                JsDecl::ExportLetInit(name, body_stmts)
            }
        }
        // The component manifest: the wired functions under their slot
        // names, plus `step: true` when update returns a (Model, outs)
        // step.
        CoreDecl::Component(n, slots, step) => {
            let mut fields: Vec<(String, JsExpr)> = slots
                .iter()
                .map(|(slot, f)| (slot.clone(), reference(env, es, f)))
                .collect();
            if *step {
                fields.push(("step".to_string(), JsExpr::Var("true".to_string())));
            }
            JsDecl::ExportConst(mangle(&n.text), JsExpr::Obj(fields))
        }
    }
}

// Names ---------------------------------------------------------------------------

const RESERVED_WORDS: &[&str] = &[
    "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
    "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function",
    "if", "import", "in", "instanceof", "let", "new", "null", "return", "static", "super",
    "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void", "while",
    "with", "yield",
    // Reserved only in strict mode; emitted ES modules are always strict,
    // so these would otherwise produce a SyntaxError at module load.
    "arguments", "eval", "implements", "interface", "package", "private", "protected", "public",
];

fn mangle(t: &str) -> String {
    if RESERVED_WORDS.contains(&t) {
        format!("{t}$")
    } else {
        t.to_string()
    }
}

/// Allocate a JS name in the innermost block: the base name if this
/// block neither declared it nor reads an enclosing binding of it,
/// otherwise the first free `$n` suffix. Sibling blocks reuse plain names
/// freely — that is what JS block scoping is for.
fn alloc_name(es: &mut Es, base: &str) -> String {
    let Some(frame) = es.frames.last_mut() else {
        maxim_panic!("emit: empty scope stack")
    };
    let free = |c: &str, frame: &Frame| !frame.taken.contains(c) && !frame.blocked.contains(c);
    let js_name = if free(base, frame) {
        base.to_string()
    } else {
        let mut k = 1;
        loop {
            let c = format!("{base}${k}");
            if free(&c, frame) {
                break c;
            }
            k += 1;
        }
    };
    frame.taken.insert(js_name.clone());
    js_name
}

fn bind_local(es: &mut Es, n: &Name) -> String {
    let js_name = alloc_name(es, &mangle(&n.text));
    es.scope.insert(n.unique, js_name.clone());
    js_name
}

fn bind_param(es: &mut Es, p: &Option<Name>) -> String {
    match p {
        Some(n) => bind_local(es, n),
        None => alloc_name(es, "_"),
    }
}

/// Run an action inside a fresh JS block scope (a function body, an
/// arrow body, an if branch, or a switch case). `blocked` is the set of
/// enclosing names the block will reference (see `scope_blocked`).
/// `scope` is restored on exit, so bindings made inside the block cannot
/// be looked up after it closes; imports, the runtime flag, and the temp
/// counter stay monotone.
fn with_scope<T>(
    env: &Env,
    es: &mut Es,
    blocked: BTreeSet<String>,
    act: impl FnOnce(&Env, &mut Es) -> T,
) -> T {
    let saved_scope = es.scope.clone();
    es.frames.push(Frame {
        taken: BTreeSet::new(),
        blocked,
    });
    let r = act(env, es);
    es.scope = saved_scope;
    es.frames.pop();
    r
}

/// Push a block scope whose blocked set is derived from the Core about
/// to be emitted inside it.
fn in_block<T>(
    env: &Env,
    es: &mut Es,
    cores: &[&Core],
    act: impl FnOnce(&Env, &mut Es) -> T,
) -> T {
    let blocked = scope_blocked(env, es, cores);
    with_scope(env, es, blocked, act)
}

/// The JS names that emitting `cores` inside a new block will reference
/// from enclosing scopes — a fresh binding in that block must not reuse
/// them. Locals resolve through the current `scope` (a miss means the
/// name is bound inside the block itself); references to this module's
/// top levels pin the bare declaration name; qualified references pin
/// the import alias, which is capitalized and never contested anyway.
fn scope_blocked(env: &Env, es: &Es, cores: &[&Core]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for c in cores {
        for n in core_refs(c) {
            match &n.sort {
                NameSort::Local => {
                    if let Some(js) = es.scope.get(&n.unique) {
                        out.insert(js.clone());
                    }
                }
                NameSort::TopVal(m) | NameSort::Ctor(m) => {
                    if m == env.module {
                        out.insert(mangle(&n.text));
                    } else {
                        out.insert(m.0.clone());
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Run a probing emission and roll back its name allocations (scope,
/// frames, temp counter). Keeping a pure `([], e)` probe result is
/// sound: any allocated names embedded in it belong to arrows that carry
/// their own block scopes, and an arrow body never references an
/// enclosing temp, so a later reallocation can only shadow legally.
fn speculate<T>(es: &mut Es, act: impl FnOnce(&mut Es) -> T) -> T {
    let saved_scope = es.scope.clone();
    let saved_frames = es.frames.clone();
    let saved_tmp = es.tmp;
    let r = act(es);
    es.scope = saved_scope;
    es.frames = saved_frames;
    es.tmp = saved_tmp;
    r
}

fn fresh_tmp(es: &mut Es) -> String {
    let n = es.tmp;
    es.tmp += 1;
    format!("${n}")
}

fn need_runtime(es: &mut Es) -> JsExpr {
    es.runtime = true;
    JsExpr::Var("$rt".to_string())
}

fn reference(env: &Env, es: &mut Es, n: &Name) -> JsExpr {
    match &n.sort {
        NameSort::TopVal(m) => {
            if m == env.module {
                JsExpr::Var(mangle(&n.text))
            } else {
                es.imports.insert(m.clone());
                JsExpr::Field(Box::new(JsExpr::Var(m.0.clone())), mangle(&n.text))
            }
        }
        _ => JsExpr::Var(
            es.scope
                .get(&n.unique)
                .cloned()
                .unwrap_or_else(|| mangle(&n.text)),
        ),
    }
}

/// Reference a component's manifest const: bare in its own module,
/// module-qualified elsewhere.
fn ref_component(env: &Env, es: &mut Es, n: &Name) -> JsExpr {
    match &n.sort {
        NameSort::Component(m) => {
            if m == env.module {
                JsExpr::Var(mangle(&n.text))
            } else {
                es.imports.insert(m.clone());
                JsExpr::Field(Box::new(JsExpr::Var(m.0.clone())), mangle(&n.text))
            }
        }
        _ => reference(env, es, n),
    }
}

fn ctor_info_of<'a>(env: &Env<'a>, n: &Name) -> &'a CtorInfo {
    env.tc
        .ctors
        .get(&n.unique)
        .unwrap_or_else(|| maxim_panic!("emit: unknown constructor"))
}

/// The JS field name for the i-th argument of constructor c.
fn ctor_field_name(env: &Env, c: &Name, i: usize) -> String {
    match &ctor_info_of(env, c).fields {
        Some(labels) if i < labels.len() => labels[i].clone(),
        _ => format!("_{i}"),
    }
}

fn ctor_value(env: &Env, es: &mut Es, n: &Name, args: Vec<JsExpr>) -> JsExpr {
    // Builtins first: they have no exported consts anywhere.
    if *n == b::ctor_true() {
        return JsExpr::Bool(true);
    }
    if *n == b::ctor_false() {
        return JsExpr::Bool(false);
    }
    if *n == b::ctor_nil() {
        return JsExpr::Obj(vec![("$".to_string(), JsExpr::Str("Nil".to_string()))]);
    }
    if args.is_empty() {
        return match &n.sort {
            NameSort::Ctor(m) => {
                if m == env.module {
                    JsExpr::Var(mangle(&n.text))
                } else {
                    es.imports.insert(m.clone());
                    JsExpr::Field(Box::new(JsExpr::Var(m.0.clone())), mangle(&n.text))
                }
            }
            _ => JsExpr::Obj(vec![("$".to_string(), JsExpr::Str(n.text.clone()))]),
        };
    }
    let mut fields = vec![("$".to_string(), JsExpr::Str(n.text.clone()))];
    for (i, a) in args.into_iter().enumerate() {
        fields.push((ctor_field_name(env, n, i), a));
    }
    JsExpr::Obj(fields)
}

fn lit_js(l: &Lit) -> JsExpr {
    match l {
        Lit::Int(n) => JsExpr::Num(n.to_string()),
        Lit::Float(d) => JsExpr::Num(d.show()),
        Lit::Str(s) => JsExpr::Str(s.clone()),
    }
}

// Destinations ----------------------------------------------------------------------

enum Dest {
    Ret,
    AssignTo(String),
}

fn finish(dest: &Dest, e: JsExpr) -> JsStmt {
    match dest {
        Dest::Ret => JsStmt::Return(e),
        Dest::AssignTo(t) => JsStmt::Assign(t.clone(), e),
    }
}

fn emit_to(env: &Env, es: &mut Es, dest: &Dest, core: &Core) -> Vec<JsStmt> {
    match core {
        Core::If(c, t, e) => {
            let (mut cs, ce) = to_expr(env, es, c);
            let t_s = in_block(env, es, &[t], |env, es| emit_to(env, es, dest, t));
            let e_s = in_block(env, es, &[e], |env, es| emit_to(env, es, dest, e));
            cs.push(JsStmt::If(ce, t_s, e_s));
            cs
        }
        Core::Match(scrut, tree, arms) => {
            let (mut ss, root) = scrut_var(env, es, scrut);
            let body = emit_tree(env, es, dest, &root, tree, arms);
            ss.extend(body);
            ss
        }
        Core::Let(n, rhs, body) => {
            let (mut rs, re) = to_expr(env, es, rhs);
            let nm = bind_local(es, n);
            rs.push(JsStmt::Const(nm, re));
            rs.extend(emit_to(env, es, dest, body));
            rs
        }
        Core::Seq(a, bb) => {
            let (mut as_, ae) = to_expr(env, es, a);
            as_.push(JsStmt::Expr(ae));
            as_.extend(emit_to(env, es, dest, bb));
            as_
        }
        _ => {
            let (mut ss, e) = to_expr(env, es, core);
            ss.push(finish(dest, e));
            ss
        }
    }
}

/// Bind the scrutinee to a variable (or reuse one), for occurrence paths.
fn scrut_var(env: &Env, es: &mut Es, scrut: &Core) -> (Vec<JsStmt>, JsExpr) {
    let (mut ss, e) = to_expr(env, es, scrut);
    match e {
        JsExpr::Var(_) => (ss, e),
        _ => {
            let t = fresh_tmp(es);
            ss.push(JsStmt::Const(t.clone(), e));
            (ss, JsExpr::Var(t))
        }
    }
}

fn to_expr(env: &Env, es: &mut Es, core: &Core) -> (Vec<JsStmt>, JsExpr) {
    match core {
        Core::Var(n) => (Vec::new(), reference(env, es, n)),
        // A stateful component tag:
        // `$rt.$instance(Counter, props, { out: h })` — trailing
        // arguments drop when absent.
        Core::Instance(comp, mp, outs) => {
            let rt = need_runtime(es);
            let comp_ref = ref_component(env, es, comp);
            let (mut ss, pe) = match mp {
                None => (Vec::new(), JsExpr::Var("null".to_string())),
                Some(p) => to_expr(env, es, p),
            };
            let mut out_pairs: Vec<(String, JsExpr)> = Vec::new();
            for (ev, h) in outs {
                let (hs, he) = to_expr(env, es, h);
                ss.extend(hs);
                out_pairs.push((ev.clone(), he));
            }
            let args = match (mp, &outs[..]) {
                (None, []) => vec![comp_ref],
                (_, []) => vec![comp_ref, pe],
                _ => vec![comp_ref, pe, JsExpr::Obj(out_pairs)],
            };
            (
                ss,
                JsExpr::Call(
                    Box::new(JsExpr::Field(Box::new(rt), "$instance".to_string())),
                    args,
                ),
            )
        }
        Core::Lit(l) => (Vec::new(), lit_js(l)),
        Core::Lam(params, body) => {
            let arrow = in_block(env, es, &[body], |env, es| {
                let ps: Vec<String> = params.iter().map(|p| bind_param(es, p)).collect();
                // Probe for an expression body; the probe's allocations
                // are rolled back so the statement-mode re-emission does
                // not see burnt names.
                let inner = speculate(es, |es| to_expr(env, es, body));
                match inner {
                    (ss, e) if ss.is_empty() => JsExpr::Arrow(ps, ArrowBody::Expr(Box::new(e))),
                    _ => JsExpr::Arrow(ps, ArrowBody::Stmts(emit_to(env, es, &Dest::Ret, body))),
                }
            });
            (Vec::new(), arrow)
        }
        Core::App(f, args) => {
            let (mut fs, fe) = to_expr(env, es, f);
            let (ass, aes) = arg_list(env, es, args);
            fs.extend(ass);
            (fs, JsExpr::Call(Box::new(fe), aes))
        }
        Core::Let(n, rhs, body) => {
            let (mut rs, re) = to_expr(env, es, rhs);
            let nm = bind_local(es, n);
            rs.push(JsStmt::Const(nm, re));
            let (bs, be) = to_expr(env, es, body);
            rs.extend(bs);
            (rs, be)
        }
        Core::Seq(a, bb) => {
            let (mut as_, ae) = to_expr(env, es, a);
            as_.push(JsStmt::Expr(ae));
            let (bs, be) = to_expr(env, es, bb);
            as_.extend(bs);
            (as_, be)
        }
        Core::Con(n, args) => {
            let (ss, aes) = arg_list(env, es, args);
            let v = ctor_value(env, es, n, aes);
            (ss, v)
        }
        Core::Update(base, fields) => {
            let (mut bs, be) = to_expr(env, es, base);
            let mut parts: Vec<(String, JsExpr)> = Vec::new();
            for (l, c) in fields {
                let (ss, e) = to_expr(env, es, c);
                bs.extend(ss);
                parts.push((l.clone(), e));
            }
            (bs, JsExpr::ObjSpread(Box::new(be), parts))
        }
        Core::Field(e, f) => {
            let (ss, je) = to_expr(env, es, e);
            (ss, JsExpr::Field(Box::new(je), f.clone()))
        }
        // A `++` chain flattens to one left-leaning `+` spine: string
        // concatenation is associative, so `a ++ (b ++ c)` prints as the
        // flat `a + b + c` a person would write. Only BConcat — JS `+` on
        // numbers is not associative, and only Maxim Strings can reach
        // BConcat.
        Core::Bin(BinOp::Concat, _, _) => {
            let spine = concat_spine(core);
            let mut ss = Vec::new();
            let mut exprs = Vec::new();
            for c in spine {
                let (s2, e2) = to_expr(env, es, c);
                ss.extend(s2);
                exprs.push(e2);
            }
            let mut it = exprs.into_iter();
            let first = it.next().expect("concat spine is never empty");
            let acc = it.fold(first, |acc, e| {
                JsExpr::Bin("+", Box::new(acc), Box::new(e))
            });
            (ss, acc)
        }
        Core::Bin(op, l, r) => {
            let (mut ls, le) = to_expr(env, es, l);
            let (rs, re) = to_expr(env, es, r);
            ls.extend(rs);
            let e = bin_js(es, *op, le, re);
            (ls, e)
        }
        Core::Un(op, e) => {
            let (ss, je) = to_expr(env, es, e);
            (
                ss,
                JsExpr::Un(
                    if *op == UnOp::Neg { "-" } else { "!" },
                    Box::new(je),
                ),
            )
        }
        Core::If(c, t, e) => {
            let (mut cs, ce) = to_expr(env, es, c);
            let t_e = speculate(es, |es| to_expr(env, es, t));
            let e_e = speculate(es, |es| to_expr(env, es, e));
            match (t_e, e_e) {
                ((ts, te), (es2, ee)) if ts.is_empty() && es2.is_empty() => {
                    (cs, JsExpr::Cond(Box::new(ce), Box::new(te), Box::new(ee)))
                }
                _ => {
                    let tmp = fresh_tmp(es);
                    let dest = Dest::AssignTo(tmp.clone());
                    let t_s = in_block(env, es, &[t], |env, es| emit_to(env, es, &dest, t));
                    let e_s = in_block(env, es, &[e], |env, es| emit_to(env, es, &dest, e));
                    cs.push(JsStmt::Let(tmp.clone()));
                    cs.push(JsStmt::If(ce, t_s, e_s));
                    (cs, JsExpr::Var(tmp))
                }
            }
        }
        Core::Match(scrut, tree, arms) => {
            let (mut ss, root) = scrut_var(env, es, scrut);
            let tmp = fresh_tmp(es);
            let body = emit_tree(env, es, &Dest::AssignTo(tmp.clone()), &root, tree, arms);
            ss.push(JsStmt::Let(tmp.clone()));
            ss.extend(body);
            (ss, JsExpr::Var(tmp))
        }
        Core::Intrinsic(i, args) => {
            let (ss, aes) = arg_list(env, es, args);
            let e = intrinsic_js(es, *i, aes);
            (ss, e)
        }
        // `?`: bind the operand (reusing it when it is already a
        // variable), return the failure early, continue with the payload
        // — the flat shape a person would write. The `return` targets the
        // enclosing JS function, which is the enclosing Maxim fn/lambda
        // by the join-point invariant (see emit_tree).
        Core::Try(kind, inner) => {
            let (mut ss, v) = scrut_var(env, es, inner);
            let bad = match kind {
                TryKind::Result => "Err",
                TryKind::Option => "None",
            };
            ss.push(JsStmt::If(
                JsExpr::Bin(
                    "===",
                    Box::new(JsExpr::Field(Box::new(v.clone()), "$".to_string())),
                    Box::new(JsExpr::Str(bad.to_string())),
                ),
                vec![JsStmt::Return(v.clone())],
                Vec::new(),
            ));
            (ss, JsExpr::Field(Box::new(v), "_0".to_string()))
        }
    }
}

fn arg_list(env: &Env, es: &mut Es, args: &[Core]) -> (Vec<JsStmt>, Vec<JsExpr>) {
    let mut ss = Vec::new();
    let mut exprs = Vec::new();
    for a in args {
        let (s2, e2) = to_expr(env, es, a);
        ss.extend(s2);
        exprs.push(e2);
    }
    (ss, exprs)
}

/// The operands of a `++` chain, left to right. Statement prefixes are
/// emitted in this order, which is the source evaluation order.
fn concat_spine(c: &Core) -> Vec<&Core> {
    match c {
        Core::Bin(BinOp::Concat, l, r) => {
            let mut out = concat_spine(l);
            out.extend(concat_spine(r));
            out
        }
        e => vec![e],
    }
}

/// Does this Core contain a `?` whose early return escapes THIS context?
/// Stops at `Lam`: a `?` under a user lambda returns from that lambda,
/// which emits as its own arrow — the correct boundary. Any code that
/// introduces a NEW emitter-side arrow (today: only join points) must
/// exclude subtrees where this is `true`, or the `return` would be
/// captured by the wrong function.
fn core_has_try(c: &Core) -> bool {
    match c {
        Core::Try(_, _) => true,
        Core::Lam(_, _) => false,
        Core::Instance(_, p, outs) => {
            p.as_deref().is_some_and(core_has_try) || outs.iter().any(|(_, e)| core_has_try(e))
        }
        Core::Var(_) | Core::Lit(_) => false,
        Core::App(f, args) => core_has_try(f) || args.iter().any(core_has_try),
        Core::Let(_, r, bb) => core_has_try(r) || core_has_try(bb),
        Core::Seq(a, bb) => core_has_try(a) || core_has_try(bb),
        Core::Con(_, args) => args.iter().any(core_has_try),
        Core::Update(bb, fs) => core_has_try(bb) || fs.iter().any(|(_, e)| core_has_try(e)),
        Core::Field(e, _) => core_has_try(e),
        Core::Bin(_, l, r) => core_has_try(l) || core_has_try(r),
        Core::Un(_, e) => core_has_try(e),
        Core::If(c2, t, e) => core_has_try(c2) || core_has_try(t) || core_has_try(e),
        Core::Match(s, _, arms) => {
            core_has_try(s)
                || arms
                    .iter()
                    .any(|(g, bb)| g.as_ref().is_some_and(core_has_try) || core_has_try(bb))
        }
        Core::Intrinsic(_, args) => args.iter().any(core_has_try),
    }
}

fn bin_js(es: &mut Es, op: BinOp, l: JsExpr, r: JsExpr) -> JsExpr {
    let bin = |o: &'static str, l: JsExpr, r: JsExpr| JsExpr::Bin(o, Box::new(l), Box::new(r));
    let eq_call = |es: &mut Es, l: JsExpr, r: JsExpr| {
        let rt = need_runtime(es);
        JsExpr::Call(
            Box::new(JsExpr::Field(Box::new(rt), "$eq".to_string())),
            vec![l, r],
        )
    };
    match op {
        BinOp::Eq => eq_call(es, l, r),
        BinOp::Neq => JsExpr::Un("!", Box::new(eq_call(es, l, r))),
        BinOp::Concat | BinOp::Add => bin("+", l, r),
        BinOp::Sub => bin("-", l, r),
        BinOp::Mul => bin("*", l, r),
        BinOp::Div => bin("/", l, r),
        BinOp::IntDiv => {
            let rt = need_runtime(es);
            JsExpr::Call(
                Box::new(JsExpr::Field(Box::new(rt), "$idiv".to_string())),
                vec![l, r],
            )
        }
        // `x % 0` is 0 (totality: NaN must not leak into Int), so `%`
        // goes through the runtime — except when the divisor is a literal
        // that is provably nonzero, where plain `%` is identical and
        // reads better.
        BinOp::Mod => match &r {
            JsExpr::Num(t) if t != "0" && t != "0.0" && !t.starts_with('-') => bin("%", l, r),
            _ => {
                let rt = need_runtime(es);
                JsExpr::Call(
                    Box::new(JsExpr::Field(Box::new(rt), "$mod".to_string())),
                    vec![l, r],
                )
            }
        },
        BinOp::Pow => bin("**", l, r),
        BinOp::Lt => bin("<", l, r),
        BinOp::Gt => bin(">", l, r),
        BinOp::Le => bin("<=", l, r),
        BinOp::Ge => bin(">=", l, r),
        BinOp::And => bin("&&", l, r),
        BinOp::Or => bin("||", l, r),
    }
}

fn intrinsic_js(es: &mut Es, i: Intrinsic, args: Vec<JsExpr>) -> JsExpr {
    let rt_call = |es: &mut Es, f: &str, args: Vec<JsExpr>| {
        let rt = need_runtime(es);
        JsExpr::Call(
            Box::new(JsExpr::Field(Box::new(rt), f.to_string())),
            args,
        )
    };
    let one = |args: Vec<JsExpr>| -> JsExpr {
        let [e] = args.try_into().unwrap_or_else(|_| {
            maxim_panic!("emit: intrinsic arity mismatch")
        });
        e
    };
    match i {
        Intrinsic::AppMount => rt_call(es, "$mount", args),
        Intrinsic::HtmlResolve => rt_call(es, "$resolve", args),
        Intrinsic::DebugLog => rt_call(es, "$log", args),
        Intrinsic::StrLength => JsExpr::Field(Box::new(one(args)), "length".to_string()),
        Intrinsic::StrReplace => {
            let [s, o, n]: [JsExpr; 3] = args
                .try_into()
                .unwrap_or_else(|_| maxim_panic!("emit: intrinsic arity mismatch"));
            JsExpr::Call(
                Box::new(JsExpr::Field(Box::new(s), "replaceAll".to_string())),
                vec![o, n],
            )
        }
        Intrinsic::IntToString | Intrinsic::FloatToString => JsExpr::Call(
            Box::new(JsExpr::Var("String".to_string())),
            vec![one(args)],
        ),
        // Int and Float are both JS numbers; widening is the identity.
        Intrinsic::FloatFromInt => one(args),
        Intrinsic::IntFromFloat | Intrinsic::FloatTrunc => rt_call(es, "$trunc", args),
        Intrinsic::FloatFloor => rt_call(es, "$floor", args),
        Intrinsic::FloatCeil => rt_call(es, "$ceil", args),
        Intrinsic::FloatRound => rt_call(es, "$round", args),
    }
}

// Decision trees ----------------------------------------------------------------------

fn occ_js(env: &Env, root: &JsExpr, occ: &Occ) -> JsExpr {
    match occ {
        Occ::Root => root.clone(),
        Occ::Arg(i, c, p) => JsExpr::Field(
            Box::new(occ_js(env, root, p)),
            ctor_field_name(env, c, *i),
        ),
    }
}

fn leaf_counts(tree: &DTree, acc: &mut BTreeMap<usize, usize>) {
    match tree {
        DTree::Leaf(i, _) => {
            *acc.entry(*i).or_insert(0) += 1;
        }
        DTree::Guard(i, _, fb) => {
            *acc.entry(*i).or_insert(0) += 1;
            leaf_counts(fb, acc);
        }
        DTree::Fail => {}
        DTree::SwitchCtor(_, branches, deflt) => {
            for (_, _, t) in branches {
                leaf_counts(t, acc);
            }
            if let Some(t) = deflt {
                leaf_counts(t, acc);
            }
        }
        DTree::SwitchLit(_, branches, deflt) => {
            for (_, t) in branches {
                leaf_counts(t, acc);
            }
            leaf_counts(deflt, acc);
        }
    }
}

fn arms_of(tree: &DTree, out: &mut BTreeSet<usize>) {
    match tree {
        DTree::Leaf(i, _) => {
            out.insert(*i);
        }
        DTree::Guard(i, _, fb) => {
            out.insert(*i);
            arms_of(fb, out);
        }
        DTree::Fail => {}
        DTree::SwitchCtor(_, branches, deflt) => {
            for (_, _, t) in branches {
                arms_of(t, out);
            }
            if let Some(t) = deflt {
                arms_of(t, out);
            }
        }
        DTree::SwitchLit(_, branches, deflt) => {
            for (_, t) in branches {
                arms_of(t, out);
            }
            arms_of(deflt, out);
        }
    }
}

fn binds_of_arm(i: usize, tree: &DTree) -> Vec<(Name, Occ)> {
    match tree {
        DTree::Leaf(j, binds) => {
            if *j == i {
                binds.clone()
            } else {
                Vec::new()
            }
        }
        DTree::Guard(j, binds, fb) => {
            if *j == i {
                binds.clone()
            } else {
                binds_of_arm(i, fb)
            }
        }
        DTree::Fail => Vec::new(),
        DTree::SwitchCtor(_, branches, deflt) => branches
            .iter()
            .map(|(_, _, t)| binds_of_arm(i, t))
            .chain(deflt.iter().map(|t| binds_of_arm(i, t)))
            .find(|b| !b.is_empty())
            .unwrap_or_default(),
        DTree::SwitchLit(_, branches, deflt) => branches
            .iter()
            .map(|(_, t)| binds_of_arm(i, t))
            .chain(std::iter::once(binds_of_arm(i, deflt)))
            .find(|b| !b.is_empty())
            .unwrap_or_default(),
    }
}

fn emit_tree(
    env: &Env,
    es: &mut Es,
    dest: &Dest,
    root: &JsExpr,
    tree: &DTree,
    arms: &[(Option<Core>, Core)],
) -> Vec<JsStmt> {
    let mut counts = BTreeMap::new();
    leaf_counts(tree, &mut counts);
    // An arm containing `?` is never shared: its early `return` must
    // target the enclosing function, and a join arrow would capture it.
    // Such arms are duplicated per leaf instead.
    let shared: Vec<usize> = counts
        .iter()
        .filter(|(i, c)| **c > 1 && !core_has_try(&arms[**i].1))
        .map(|(i, _)| *i)
        .collect();
    // Join points: arms reachable from several leaves become local
    // arrows. Only the body is shared; a guard is tested at each site
    // (its fallback tree differs per site). The arrow is its own block
    // scope; its body only ever references its parameters and enclosing
    // bindings, never the match root, so the root is not blocked here.
    let mut join_map: BTreeMap<usize, String> = BTreeMap::new();
    let mut join_stmts: Vec<JsStmt> = Vec::new();
    for i in shared {
        let (params, jp_body) = in_block(env, es, &[&arms[i].1], |env, es| {
            let ps: Vec<String> = binds_of_arm(i, tree)
                .iter()
                .map(|(n, _)| bind_local(es, n))
                .collect();
            let body = emit_to(env, es, &Dest::Ret, &arms[i].1);
            (ps, body)
        });
        let jp = fresh_tmp(es);
        join_map.insert(i, jp.clone());
        join_stmts.push(JsStmt::Const(
            jp,
            JsExpr::Arrow(params, ArrowBody::Stmts(jp_body)),
        ));
    }
    let mut out = join_stmts;
    out.extend(walk(env, es, dest, root, &join_map, tree, arms));
    out
}

/// The root is always a variable (`scrut_var` guarantees it); its name
/// is blocked in every branch scope, because branch code keeps reading
/// it through occurrence paths (`root._0` …).
fn root_text(root: &JsExpr) -> String {
    match root {
        JsExpr::Var(t) => t.clone(),
        _ => maxim_panic!("emit: match root is not a variable"),
    }
}

/// A switch case or if branch: a fresh JS block emitting the subtree.
fn in_branch<T>(
    env: &Env,
    es: &mut Es,
    root: &JsExpr,
    t: &DTree,
    arms: &[(Option<Core>, Core)],
    act: impl FnOnce(&Env, &mut Es) -> T,
) -> T {
    // The arms (guard and body Cores) reachable in a subtree — the
    // blocked-set input for the JS block a branch emits into.
    let mut reachable = BTreeSet::new();
    arms_of(t, &mut reachable);
    let mut cores: Vec<&Core> = Vec::new();
    for i in &reachable {
        if let Some(g) = &arms[*i].0 {
            cores.push(g);
        }
        cores.push(&arms[*i].1);
    }
    let mut blocked = scope_blocked(env, es, &cores);
    blocked.insert(root_text(root));
    with_scope(env, es, blocked, act)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    env: &Env,
    es: &mut Es,
    dest: &Dest,
    root: &JsExpr,
    join_map: &BTreeMap<usize, String>,
    tree: &DTree,
    arms: &[(Option<Core>, Core)],
) -> Vec<JsStmt> {
    let with_break = |body: Vec<JsStmt>| -> Vec<JsStmt> {
        match dest {
            Dest::Ret => body,
            Dest::AssignTo(_) => match body.last() {
                Some(JsStmt::Throw(_)) | Some(JsStmt::Return(_)) => body,
                _ => {
                    let mut b = body;
                    b.push(JsStmt::Break);
                    b
                }
            },
        }
    };
    match tree {
        DTree::Fail => vec![JsStmt::Throw(format!(
            "unreachable match in {}",
            env.module.0
        ))],
        DTree::Leaf(i, binds) => match join_map.get(i) {
            Some(jp) => vec![finish(
                dest,
                JsExpr::Call(
                    Box::new(JsExpr::Var(jp.clone())),
                    binds.iter().map(|(_, occ)| occ_js(env, root, occ)).collect(),
                ),
            )],
            None => {
                let mut out = Vec::new();
                for (n, occ) in binds {
                    let nm = bind_local(es, n);
                    out.push(JsStmt::Const(nm, occ_js(env, root, occ)));
                }
                out.extend(emit_to(env, es, dest, &arms[*i].1));
                out
            }
        },
        DTree::Guard(i, binds, fb) => {
            let mut bound: Vec<(String, &Occ)> = Vec::new();
            let mut out = Vec::new();
            for (n, occ) in binds {
                let nm = bind_local(es, n);
                out.push(JsStmt::Const(nm.clone(), occ_js(env, root, occ)));
                bound.push((nm, occ));
            }
            let (gs, ge) = match &arms[*i].0 {
                Some(g) => to_expr(env, es, g),
                None => maxim_panic!("emit: DGuard on an unguarded arm"),
            };
            let then_s = match join_map.get(i) {
                Some(jp) => vec![finish(
                    dest,
                    JsExpr::Call(
                        Box::new(JsExpr::Var(jp.clone())),
                        bound
                            .iter()
                            .map(|(nm, _)| JsExpr::Var(nm.clone()))
                            .collect(),
                    ),
                )],
                None => in_block(env, es, &[&arms[*i].1], |env, es| {
                    emit_to(env, es, dest, &arms[*i].1)
                }),
            };
            let else_s = in_branch(env, es, root, fb, arms, |env, es| {
                walk(env, es, dest, root, join_map, fb, arms)
            });
            out.extend(gs);
            out.push(JsStmt::If(ge, then_s, else_s));
            out
        }
        DTree::SwitchCtor(occ, branches, deflt) => {
            let all_bool = branches
                .iter()
                .all(|(c, _, _)| *c == b::ctor_true() || *c == b::ctor_false());
            if all_bool {
                // Bool: an if on the value itself.
                let branch_or = |env: &Env, es: &mut Es, want: &Name| -> Vec<JsStmt> {
                    match branches.iter().find(|(c, _, _)| c == want) {
                        Some((_, _, t)) => in_branch(env, es, root, t, arms, |env, es| {
                            walk(env, es, dest, root, join_map, t, arms)
                        }),
                        None => match deflt {
                            Some(t) => in_branch(env, es, root, t, arms, |env, es| {
                                walk(env, es, dest, root, join_map, t, arms)
                            }),
                            None => vec![JsStmt::Throw("unreachable match".to_string())],
                        },
                    }
                };
                let t_s = branch_or(env, es, &b::ctor_true());
                let f_s = branch_or(env, es, &b::ctor_false());
                vec![JsStmt::If(occ_js(env, root, occ), t_s, f_s)]
            } else {
                let mut cases: Vec<(JsExpr, Vec<JsStmt>)> = Vec::new();
                for (c, _, t) in branches {
                    let body = in_branch(env, es, root, t, arms, |env, es| {
                        walk(env, es, dest, root, join_map, t, arms)
                    });
                    cases.push((JsExpr::Str(c.text.clone()), with_break(body)));
                }
                let deflt_s = deflt.as_ref().map(|t| {
                    with_break(in_branch(env, es, root, t, arms, |env, es| {
                        walk(env, es, dest, root, join_map, t, arms)
                    }))
                });
                vec![JsStmt::Switch(
                    JsExpr::Field(Box::new(occ_js(env, root, occ)), "$".to_string()),
                    cases,
                    deflt_s,
                )]
            }
        }
        DTree::SwitchLit(occ, branches, deflt) => {
            let mut cases: Vec<(JsExpr, Vec<JsStmt>)> = Vec::new();
            for (l, t) in branches {
                let body = in_branch(env, es, root, t, arms, |env, es| {
                    walk(env, es, dest, root, join_map, t, arms)
                });
                cases.push((lit_js(l), with_break(body)));
            }
            let deflt_s = with_break(in_branch(env, es, root, deflt, arms, |env, es| {
                walk(env, es, dest, root, join_map, deflt, arms)
            }));
            vec![JsStmt::Switch(occ_js(env, root, occ), cases, Some(deflt_s))]
        }
    }
}
