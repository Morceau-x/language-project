//! Eager unification with blame: every call site says WHY two types must
//! match (`Reason`), so a failure renders an Elm-grade message at the
//! exact node where expectations collided — the core of the error-quality
//! story.

use crate::ast::{BinOp, UnOp};
use crate::diag::{Diagnostic, dtext};
use crate::span::Span;
use crate::types::ty::*;

/// Why two types must be equal. Carries the span to blame and enough
/// context to phrase the failure in the user's terms.
#[derive(Clone, Debug)]
pub enum Reason {
    /// Arg index (1-based), a description of the callee, span of the arg.
    CallArg(usize, String, Span),
    IfCond(Span),
    /// Span of the offending branch.
    IfBranches(Span),
    /// Arm index (1-based), span of the arm body.
    MatchArm(usize, Span),
    /// Element index (1-based), span of the element.
    ListElem(usize, Span),
    /// Span of the `..spread` expression.
    ListSpread(Span),
    BinOp(BinOp, Span),
    Unary(UnOp, Span),
    /// Function name and the span of its body result.
    RetAnn(String, Span),
    /// A `let`'s annotation vs its right-hand side.
    LetAnn(Span),
    /// Pattern vs scrutinee.
    Pattern(Span),
    /// Record-construction field: label, constructor name, arg span.
    RecField(String, String, Span),
    /// Field access resolution: field name, access span.
    FieldAccess(String, Span),
    /// Record update: constructor name, span of the `..base` expression.
    UpdateBase(String, Span),
    /// The operand of a `?`; span of the `?` token.
    TryOperand(Span),
    /// A `?`'s early return vs the enclosing return type: the enclosing
    /// function's name (None for a lambda), span of the `?` token.
    TryRet(Option<String>, Span),
    /// A function or lambda body vs a return type that a `?` inside it
    /// constrained; span of the body.
    LamRet(Span),
    /// A `${…}` splice must be a String; span of the spliced expression.
    Interp(Span),
    /// An attribute of an intrinsic element: name, span of the value.
    JsxAttr(String, Span),
    /// An `on:` handler: event name, span of the handler expression.
    JsxHandler(String, Span),
    /// A `{…}` child must be an element; span of the spliced expression.
    JsxSplice(Span),
    /// A `{..…}` child spreads a list of elements; span of the expression.
    JsxSpread(Span),
    /// A child element's message type vs its parent's; span of the child.
    JsxChild(Span),
}

impl Reason {
    pub fn span(&self) -> Span {
        match self {
            Reason::CallArg(_, _, sp)
            | Reason::IfCond(sp)
            | Reason::IfBranches(sp)
            | Reason::MatchArm(_, sp)
            | Reason::ListElem(_, sp)
            | Reason::ListSpread(sp)
            | Reason::BinOp(_, sp)
            | Reason::Unary(_, sp)
            | Reason::RetAnn(_, sp)
            | Reason::LetAnn(sp)
            | Reason::Pattern(sp)
            | Reason::RecField(_, _, sp)
            | Reason::FieldAccess(_, sp)
            | Reason::UpdateBase(_, sp)
            | Reason::TryOperand(sp)
            | Reason::TryRet(_, sp)
            | Reason::LamRet(sp)
            | Reason::Interp(sp)
            | Reason::JsxAttr(_, sp)
            | Reason::JsxHandler(_, sp)
            | Reason::JsxSplice(sp)
            | Reason::JsxSpread(sp)
            | Reason::JsxChild(sp) => *sp,
        }
    }
}

/// `unify(arena, diags, reason, got, want)`: make `got` equal to `want`,
/// reporting a diagnostic on failure (and continuing — the error type
/// absorbs the rest).
pub fn unify(
    arena: &mut MetaArena,
    diags: &mut Vec<Diagnostic>,
    reason: &Reason,
    got: &Type,
    want: &Type,
) {
    let a = arena.zonk(got);
    let b = arena.zonk(want);
    match (&a, &b) {
        (Type::Error, _) | (_, Type::Error) => {}
        (Type::Meta(m), t) => bind_meta(arena, diags, reason, true, *m, t),
        (t, Type::Meta(m)) => bind_meta(arena, diags, reason, false, *m, t),
        (Type::Var(v1), Type::Var(v2)) if v1 == v2 => {}
        (Type::Con(c1, args_a), Type::Con(c2, args_b))
            if c1 == c2 && args_a.len() == args_b.len() =>
        {
            for (x, y) in args_a.iter().zip(args_b) {
                unify(arena, diags, reason, x, y);
            }
        }
        // Parameter labels are advisory metadata: unification compares
        // only the types, so differently-named (or unnamed) parameters
        // never block assignability.
        (Type::Fun(ps_a, r1), Type::Fun(ps_b, r2)) if ps_a.len() == ps_b.len() => {
            for (x, y) in ps_a.iter().zip(ps_b) {
                unify(arena, diags, reason, &x.ty, &y.ty);
            }
            unify(arena, diags, reason, r1, r2);
        }
        _ => mismatch(arena, diags, reason, &a, &b),
    }
}

fn bind_meta(
    arena: &mut MetaArena,
    diags: &mut Vec<Diagnostic>,
    reason: &Reason,
    meta_is_got: bool,
    m: MetaId,
    t: &Type,
) {
    if let Type::Meta(m2) = t {
        if *m2 == m {
            return;
        }
    }
    if occurs(arena, m, t) {
        let rendered = render_types(arena, &[Type::Meta(m), t.clone()]);
        diags.push(
            Diagnostic::error(
                "E3004",
                "INFINITE TYPE",
                reason.span(),
                "this would be an infinite type",
            )
            .with_body(vec![dtext(format!(
                "Unifying `{}` with `{}` would build a type that contains itself, forever.",
                rendered[0], rendered[1]
            ))]),
        );
        return;
    }
    match arena.flavor(m) {
        Flavor::Star => {
            promote(arena, m, t);
            arena.set(m, MetaContent::Bound(t.clone()));
        }
        Flavor::Num => match t {
            Type::Con(c, args) if args.is_empty() && (*c == tc_int() || *c == tc_float()) => {
                arena.set(m, MetaContent::Bound(t.clone()));
            }
            Type::Var(v) if v.flavor == Flavor::Num => {
                arena.set(m, MetaContent::Bound(t.clone()));
            }
            Type::Meta(m2) => {
                if arena.flavor(*m2) == Flavor::Num {
                    promote(arena, m, t);
                    arena.set(m, MetaContent::Bound(t.clone()));
                } else {
                    // A Star meta is less specific: point it at the Num one.
                    promote(arena, *m2, &Type::Meta(m));
                    arena.set(*m2, MetaContent::Bound(Type::Meta(m)));
                }
            }
            // Outside operator contexts a flavor failure reads better as
            // an ordinary mismatch with the meta rendered as `number`.
            _ => match reason {
                Reason::BinOp(_, _) | Reason::Unary(_, _) => num_mismatch(arena, diags, reason, t),
                _ => {
                    if meta_is_got {
                        mismatch(arena, diags, reason, &Type::Meta(m), t);
                    } else {
                        mismatch(arena, diags, reason, t, &Type::Meta(m));
                    }
                }
            },
        },
    }
}

/// Standard level adjustment: metas inside `t` may not outlive `m`'s level.
fn promote(arena: &mut MetaArena, m: MetaId, t: &Type) {
    let m_level = match &arena.cell(m).content {
        MetaContent::Unbound(l) => *l,
        MetaContent::Bound(_) => TOP_LEVEL,
    };
    let t2 = arena.zonk(t);
    for m2 in free_metas(&t2) {
        if let MetaContent::Unbound(l2) = arena.cell(m2).content {
            if l2 > m_level {
                arena.set(m2, MetaContent::Unbound(m_level));
            }
        }
    }
}

fn occurs(arena: &mut MetaArena, m: MetaId, t: &Type) -> bool {
    let t2 = arena.zonk(t);
    free_metas(&t2).contains(&m)
}

fn num_mismatch(arena: &mut MetaArena, diags: &mut Vec<Diagnostic>, reason: &Reason, t: &Type) {
    let tr = render_type(arena, t);
    let context = match reason {
        Reason::BinOp(op, _) => format!("`{}`", op.text()),
        Reason::Unary(UnOp::Neg, _) => "`-`".to_string(),
        _ => "this".to_string(),
    };
    let num_hints: Vec<String> = match t {
        Type::Con(c, args) if args.is_empty() && *c == tc_string() => {
            vec!["To build strings, use interpolation: `\"${a}${b}\"`.".to_string()]
        }
        Type::Var(v) if v.flavor == Flavor::Star => {
            vec![format!(
                "`{}` comes from a signature: the caller chooses what it is.",
                v.text
            )]
        }
        _ => Vec::new(),
    };
    diags.push(
        Diagnostic::error(
            "E3003",
            "NUMBER NEEDED",
            reason.span(),
            format!("this is `{tr}`"),
        )
        .with_body(vec![dtext(format!(
            "{context} works on Int or Float, but this is `{tr}`."
        ))])
        .with_hints(num_hints),
    );
}

fn mismatch(
    arena: &mut MetaArena,
    diags: &mut Vec<Diagnostic>,
    reason: &Reason,
    got: &Type,
    want: &Type,
) {
    let rendered = render_types(arena, &[got.clone(), want.clone()]);
    let (got_r, want_r) = (&rendered[0], &rendered[1]);

    // The skolem story: a rigid var on either side means the caller
    // picks the type.
    let rigid_hint = || -> Vec<String> {
        let rigid_text = |v: &TyVar| -> Vec<String> {
            if v.flavor == Flavor::Num {
                Vec::new()
            } else {
                vec![format!(
                    "`{}` comes from a signature: the caller chooses what it is, so the body can't pin it down.",
                    v.text
                )]
            }
        };
        match (got, want) {
            (Type::Var(v), _) => rigid_text(v),
            (_, Type::Var(v)) => rigid_text(v),
            _ => Vec::new(),
        }
    };
    let numeric_literal_hint = |op: BinOp| -> Vec<String> {
        use BinOp::*;
        if matches!(op, Add | Sub | Mul | Div | IntDiv | Mod | Pow)
            && ((got_r == "Int" && want_r == "Float") || (got_r == "Float" && want_r == "Int"))
        {
            vec![
                "Maxim never converts numbers implicitly. Write the literal with a decimal point (`2.0`) or change the other side."
                    .to_string(),
            ]
        } else {
            Vec::new()
        }
    };
    let jsx_splice_hint = |g: &str| -> Vec<String> {
        if g == "String" {
            vec!["A `String` splices as text: `${…}`.".to_string()]
        } else {
            Vec::new()
        }
    };
    let interp_hint = |g: &str| -> Vec<String> {
        match g {
            "Int" => vec!["Convert it first: `Int.toString(n)`.".to_string()],
            "Float" => vec!["Convert it first: `Float.toString(x)`.".to_string()],
            "number" => {
                vec!["Convert it first: `Int.toString(n)` or `Float.toString(x)`.".to_string()]
            }
            _ => Vec::new(),
        }
    };

    let (title, label, body, hints): (&str, String, String, Vec<String>) = match reason {
        Reason::CallArg(i, callee, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!(
                "The {} argument of {callee} should be `{want_r}`.",
                ordinal(*i)
            ),
            rigid_hint(),
        ),
        Reason::IfCond(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            "An `if` condition must be a `Bool`.".to_string(),
            Vec::new(),
        ),
        Reason::IfBranches(_) => (
            "BRANCH MISMATCH",
            format!("this branch produces `{got_r}`"),
            format!(
                "Both branches of an `if` must produce the same type; the other branch produces `{want_r}`."
            ),
            Vec::new(),
        ),
        Reason::MatchArm(i, _) => (
            "ARM MISMATCH",
            format!("arm {i} produces `{got_r}`"),
            format!(
                "Every arm of a `match` must produce the same type; the arms before produce `{want_r}`."
            ),
            Vec::new(),
        ),
        Reason::ListElem(i, _) => (
            "LIST MISMATCH",
            format!("element {i} is `{got_r}`"),
            format!(
                "All elements of a list must share one type; the elements before are `{want_r}`."
            ),
            Vec::new(),
        ),
        Reason::ListSpread(_) => (
            "LIST MISMATCH",
            format!("this is `{got_r}`"),
            format!("The `..` tail must be `{want_r}`."),
            Vec::new(),
        ),
        Reason::BinOp(op, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("`{}` needs `{want_r}` here.", op.text()),
            numeric_literal_hint(*op),
        ),
        Reason::Unary(op, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!(
                "`{}` needs `{want_r}`.",
                match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                }
            ),
            Vec::new(),
        ),
        Reason::RetAnn(name, _) => (
            "RETURN MISMATCH",
            format!("the body produces `{got_r}`"),
            format!("The signature of `{name}` declares the return type `{want_r}`."),
            rigid_hint(),
        ),
        Reason::LetAnn(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("The annotation says `{want_r}`."),
            rigid_hint(),
        ),
        Reason::Pattern(_) => (
            "PATTERN MISMATCH",
            format!("this pattern matches `{got_r}`"),
            format!("But the value being matched is `{want_r}`."),
            Vec::new(),
        ),
        Reason::RecField(lbl, ctor, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("The field `{lbl}` of `{ctor}` is `{want_r}`."),
            Vec::new(),
        ),
        Reason::FieldAccess(fld, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("The field `{fld}` here should be `{want_r}`."),
            Vec::new(),
        ),
        Reason::UpdateBase(ctor, _) => (
            "UPDATE MISMATCH",
            format!("this is `{got_r}`"),
            format!("`{ctor}(..base, …)` updates `{want_r}`, so the base must be one."),
            Vec::new(),
        ),
        Reason::TryOperand(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("`?` unwraps `{want_r}`, so its operand must be one."),
            Vec::new(),
        ),
        Reason::TryRet(m_name, _) => (
            "RETURN MISMATCH",
            format!("a failing `?` here returns `{got_r}`"),
            format!(
                "The failure escapes to the caller, so {} must return `{got_r}` — but it returns `{want_r}`.",
                match m_name {
                    Some(n) => format!("`{n}`"),
                    None => "the enclosing function".to_string(),
                }
            ),
            vec![
                "A function that uses `?` returns the failure it unwraps: a `Result` with the same error type, or an `Option`."
                    .to_string(),
            ],
        ),
        Reason::LamRet(_) => (
            "RETURN MISMATCH",
            format!("the body produces `{got_r}`"),
            format!("But `?` inside makes the enclosing function return `{want_r}`."),
            Vec::new(),
        ),
        Reason::Interp(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            "Everything spliced into a string with `${…}` must be a `String`.".to_string(),
            interp_hint(got_r),
        ),
        Reason::JsxAttr(name, _) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("The attribute `{name}` of an element takes a `String`."),
            interp_hint(got_r),
        ),
        Reason::JsxHandler(ev, _) => (
            "HANDLER MISMATCH",
            format!("this handler carries `{got_r}`"),
            format!(
                "Every `on:` handler of one element carries the same message type; `on:{ev}` here needs `{want_r}`."
            ),
            Vec::new(),
        ),
        Reason::JsxSplice(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("A `{{…}}` child must be `{want_r}`."),
            jsx_splice_hint(got_r),
        ),
        Reason::JsxSpread(_) => (
            "TYPE MISMATCH",
            format!("this is `{got_r}`"),
            format!("A `{{..…}}` child spreads `{want_r}` into the element."),
            Vec::new(),
        ),
        Reason::JsxChild(_) => (
            "ELEMENT MISMATCH",
            format!("this child is `{got_r}`"),
            format!("A child element shares its parent's message type: `{want_r}`."),
            Vec::new(),
        ),
    };

    // Two function types of different arities: the writer may have meant
    // a partial application, which only happens when the callee's full
    // type is known at the call.
    let arity_hint: Vec<String> = match (got, want) {
        (Type::Fun(ps_a, _), Type::Fun(ps_b, _)) if ps_a.len() != ps_b.len() => vec![
            "Maxim applies a function to all its arguments at once unless the callee's full type is already known. If you meant a partial application, annotate the function, like `f: (Int, Int) -> Int`."
                .to_string(),
        ],
        _ => Vec::new(),
    };

    let mut all_hints = hints;
    all_hints.extend(arity_hint);
    diags.push(
        Diagnostic::error("E3002", title, reason.span(), label)
            .with_body(vec![dtext(body)])
            .with_hints(all_hints),
    );
}

pub fn ordinal(n: usize) -> String {
    let suffix = match n % 100 {
        11 | 12 | 13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}
