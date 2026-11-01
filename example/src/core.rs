//! The Core IR: a small, sugar-free expression language sitting between
//! the typed surface AST and the JS backend. Pipes, list literals,
//! labeled construction, bare constructors, and blocks are gone by the
//! time a program is here; matches carry their compiled decision trees.

pub mod lower;

use std::collections::BTreeSet;

use crate::ast::{BinOp, Lit, TryKind, UnOp};
use crate::match_analysis::DTree;
use crate::names::{ModuleName, Name};

#[derive(Clone, Debug)]
pub enum Core {
    Var(Name),
    Lit(Lit),
    /// N-ary lambda; `None` parameters are ignored (`_`).
    Lam(Vec<Option<Name>>, Box<Core>),
    App(Box<Core>, Vec<Core>),
    Let(Name, Box<Core>, Box<Core>),
    /// Evaluate and discard, then continue (a block statement).
    Seq(Box<Core>, Box<Core>),
    /// Saturated constructor application (record fields in declaration
    /// order).
    Con(Name, Vec<Core>),
    /// Record update: base plus labeled replacements in written order.
    Update(Box<Core>, Vec<(String, Core)>),
    Field(Box<Core>, String),
    Bin(BinOp, Box<Core>, Box<Core>),
    Un(UnOp, Box<Core>),
    If(Box<Core>, Box<Core>, Box<Core>),
    /// Scrutinee, its decision tree, and the arms the tree's leaves
    /// index into: optional guard plus body.
    Match(Box<Core>, DTree, Vec<(Option<Core>, Core)>),
    Intrinsic(Intrinsic, Vec<Core>),
    /// `expr?`: unwrap the success payload or early-return the failure
    /// from the enclosing function.
    Try(TryKind, Box<Core>),
    /// A stateful component tag: the component's manifest, its Props
    /// value (when it has props), and its out-event handlers by event
    /// name. Emits as a runtime instance node.
    Instance(Name, Option<Box<Core>>, Vec<(String, Core)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    AppMount,
    HtmlResolve,
    DebugLog,
    StrLength,
    StrReplace,
    IntToString,
    FloatToString,
    FloatFromInt,
    IntFromFloat,
    FloatTrunc,
    FloatFloor,
    FloatCeil,
    FloatRound,
}

impl Intrinsic {
    /// Every intrinsic is fully applied in Core; a bare reference is
    /// eta-expanded to this arity.
    pub fn arity(self) -> usize {
        match self {
            Intrinsic::StrReplace => 3,
            _ => 1,
        }
    }
}

/// Every `Name` the tree references through a variable, a constructor,
/// or a guard/arm body. Binders are deliberately not subtracted: uniques
/// are globally fresh after renaming, so a binder can never collide with
/// an enclosing reference, and callers only intersect the result with
/// names already in scope — the over-approximation is exact for that use.
pub fn core_refs(c: &Core) -> BTreeSet<Name> {
    fn go(c: &Core, out: &mut BTreeSet<Name>) {
        match c {
            Core::Var(n) => {
                out.insert(n.clone());
            }
            Core::Lit(_) => {}
            Core::Lam(_, b) => go(b, out),
            Core::App(f, args) => {
                go(f, out);
                for a in args {
                    go(a, out);
                }
            }
            Core::Let(_, r, b) => {
                go(r, out);
                go(b, out);
            }
            Core::Seq(a, b) => {
                go(a, out);
                go(b, out);
            }
            Core::Con(n, args) => {
                out.insert(n.clone());
                for a in args {
                    go(a, out);
                }
            }
            Core::Update(b, fs) => {
                go(b, out);
                for (_, e) in fs {
                    go(e, out);
                }
            }
            Core::Field(e, _) => go(e, out),
            Core::Bin(_, l, r) => {
                go(l, out);
                go(r, out);
            }
            Core::Un(_, e) => go(e, out),
            Core::If(c2, t, e) => {
                go(c2, out);
                go(t, out);
                go(e, out);
            }
            Core::Match(s, _, arms) => {
                go(s, out);
                for (g, b) in arms {
                    if let Some(g2) = g {
                        go(g2, out);
                    }
                    go(b, out);
                }
            }
            Core::Intrinsic(_, args) => {
                for a in args {
                    go(a, out);
                }
            }
            Core::Try(_, e) => go(e, out),
            Core::Instance(n, p, outs) => {
                out.insert(n.clone());
                if let Some(p2) = p {
                    go(p2, out);
                }
                for (_, e) in outs {
                    go(e, out);
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    go(c, &mut out);
    out
}

#[derive(Clone, Debug)]
pub enum CoreDecl {
    Fn(Name, Vec<Option<Name>>, Core),
    Let(Name, Core),
    /// `component Counter(init: init, update: update, view: view)` —
    /// the manifest object: slot names and the wired functions, plus
    /// whether `update` returns a step (the runtime unpacks the pair).
    Component(Name, Vec<(String, Name)>, bool),
}

#[derive(Clone, Debug)]
pub struct CoreModule {
    pub name: ModuleName,
    pub decls: Vec<CoreDecl>,
}

#[cfg(test)]
mod tests {
    //! Ported from `compiler/test/CoreTests.hs`. `core_refs` feeds the
    //! JS emitter's blocked sets (which enclosing names a block
    //! references), so what matters is: every reference position is
    //! collected, and binders are NOT subtracted — uniques are globally
    //! fresh, so callers intersecting with in-scope names get an exact
    //! answer.

    use super::*;
    use crate::ast::{BinOp, Lit};
    use crate::lexer::token::IntLit;
    use crate::match_analysis::DTree;
    use crate::names::{ModuleName, NameSort};
    use crate::span::{FileId, Pos, Span};
    use crate::unique::Unique;

    fn mk(u: i64, t: &str, s: NameSort) -> Name {
        Name {
            unique: Unique(u),
            text: t.to_string(),
            sort: s,
            def_span: Span::new(FileId(0), Pos::new(1, 1, 0), Pos::new(1, 1, 0)),
        }
    }

    fn fn_f() -> Name {
        mk(1, "f", NameSort::TopVal(ModuleName("Main".to_string())))
    }

    fn ctor_some() -> Name {
        mk(2, "Some", NameSort::Ctor(ModuleName("Option".to_string())))
    }

    fn local(u: i64, t: &str) -> Name {
        mk(u, t, NameSort::Local)
    }

    #[test]
    fn collects_variables_constructors_and_nested_positions() {
        let e = Core::App(
            Box::new(Core::Var(fn_f())),
            vec![
                Core::Con(
                    ctor_some(),
                    vec![Core::Bin(
                        BinOp::Add,
                        Box::new(Core::Var(local(3, "x"))),
                        Box::new(Core::Lit(Lit::Int(IntLit::from_raw("1")))),
                    )],
                ),
                Core::Update(
                    Box::new(Core::Var(local(4, "y"))),
                    vec![("age".to_string(), Core::Var(local(3, "x")))],
                ),
            ],
        );
        let want: BTreeSet<Name> = [fn_f(), ctor_some(), local(3, "x"), local(4, "y")].into();
        assert_eq!(core_refs(&e), want);
    }

    #[test]
    fn walks_match_scrutinee_guards_and_arm_bodies() {
        let m = Core::Match(
            Box::new(Core::Var(local(3, "x"))),
            DTree::Leaf(0, Vec::new()),
            vec![
                (Some(Core::Var(local(6, "ok"))), Core::Var(local(4, "y"))),
                (
                    None,
                    Core::Lam(vec![Some(local(5, "z"))], Box::new(Core::Var(fn_f()))),
                ),
            ],
        );
        let want: BTreeSet<Name> = [local(3, "x"), local(6, "ok"), local(4, "y"), fn_f()].into();
        assert_eq!(core_refs(&m), want);
    }

    #[test]
    fn binders_are_not_subtracted() {
        let e = Core::Let(
            local(3, "x"),
            Box::new(Core::Var(fn_f())),
            Box::new(Core::Var(local(3, "x"))),
        );
        let want: BTreeSet<Name> = [fn_f(), local(3, "x")].into();
        assert_eq!(core_refs(&e), want);
    }
}
