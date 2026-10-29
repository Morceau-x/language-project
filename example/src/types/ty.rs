//! The type representation: rigid variables, metavariables with Rémy
//! levels for generalization, and the `Num` flavor implementing Elm-style
//! overloaded numerics (a Num variable unifies only with Int, Float, or
//! another Num variable).
//!
//! Where the Haskell keeps each metavariable in an `IORef`, the Rust port
//! keeps all cells in one [`MetaArena`] owned by the checker: `TMeta`
//! carries a [`MetaId`], cloning a `Type` shares metas by id (exactly the
//! IORef aliasing), and zonk/bind/generalize mutate through `&mut`.

use std::collections::BTreeMap;

use crate::builtin as b;
use crate::names::Name;
use crate::unique::Unique;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level(pub u32);

pub const TOP_LEVEL: Level = Level(0);

impl Level {
    pub fn bump(self) -> Level {
        Level(self.0 + 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Star,
    Num,
}

/// A rigid type variable: from a user annotation (skolem — the caller
/// chooses it) or created by generalization.
#[derive(Clone, Debug)]
pub struct TyVar {
    pub unique: Unique,
    pub text: String,
    pub flavor: Flavor,
}

impl PartialEq for TyVar {
    fn eq(&self, other: &Self) -> bool {
        self.unique == other.unique
    }
}

impl Eq for TyVar {}

impl PartialOrd for TyVar {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TyVar {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.unique.cmp(&other.unique)
    }
}

/// A unification variable: an index into the checker's [`MetaArena`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetaId(pub u32);

#[derive(Clone, Debug)]
pub enum MetaContent {
    Unbound(Level),
    Bound(Type),
}

#[derive(Clone, Debug)]
pub struct MetaCell {
    /// For rendering (`a`, `number`, …) and debug display only.
    pub unique: Unique,
    pub flavor: Flavor,
    pub content: MetaContent,
}

/// All metavariable cells of one module check.
#[derive(Debug, Default)]
pub struct MetaArena {
    cells: Vec<MetaCell>,
}

impl MetaArena {
    pub fn new() -> MetaArena {
        MetaArena { cells: Vec::new() }
    }

    pub fn alloc(&mut self, unique: Unique, flavor: Flavor, level: Level) -> MetaId {
        let id = MetaId(self.cells.len() as u32);
        self.cells.push(MetaCell {
            unique,
            flavor,
            content: MetaContent::Unbound(level),
        });
        id
    }

    pub fn cell(&self, id: MetaId) -> &MetaCell {
        &self.cells[id.0 as usize]
    }

    pub fn flavor(&self, id: MetaId) -> Flavor {
        self.cell(id).flavor
    }

    pub fn set(&mut self, id: MetaId, content: MetaContent) {
        self.cells[id.0 as usize].content = content;
    }

    /// Resolve bound metavariables (with path compression).
    pub fn zonk(&mut self, ty: &Type) -> Type {
        match ty {
            Type::Meta(m) => match self.cell(*m).content.clone() {
                MetaContent::Bound(b) => {
                    let b2 = self.zonk(&b);
                    self.set(*m, MetaContent::Bound(b2.clone()));
                    b2
                }
                MetaContent::Unbound(_) => ty.clone(),
            },
            Type::Con(c, args) => {
                Type::Con(c.clone(), args.iter().map(|a| self.zonk(a)).collect())
            }
            Type::Fun(params, ret) => Type::Fun(
                params
                    .iter()
                    .map(|p| TParam {
                        label: p.label.clone(),
                        ty: self.zonk(&p.ty),
                    })
                    .collect(),
                Box::new(self.zonk(ret)),
            ),
            other => other.clone(),
        }
    }

    pub fn zonk_scheme(&mut self, s: &Scheme) -> Scheme {
        Scheme {
            vars: s.vars.clone(),
            ty: self.zonk(&s.ty),
        }
    }
}

/// Unbound metavariables of a zonked type, left to right, deduplicated.
pub fn free_metas(ty: &Type) -> Vec<MetaId> {
    fn go(ty: &Type, out: &mut Vec<MetaId>) {
        match ty {
            Type::Meta(m) => {
                if !out.contains(m) {
                    out.push(*m);
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
            Type::Var(_) | Type::Error => {}
        }
    }
    let mut out = Vec::new();
    go(ty, &mut out);
    out
}

#[derive(Clone, Debug)]
pub struct TyCon {
    pub name: Name,
    pub arity: usize,
}

impl PartialEq for TyCon {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TyCon {}

#[derive(Clone, Debug)]
pub enum Type {
    Con(TyCon, Vec<Type>),
    /// N-ary: `(Int, Int) -> Int` is not `(Int) -> (Int) -> Int`. Makes
    /// arity a first-class error.
    Fun(Vec<TParam>, Box<Type>),
    Var(TyVar),
    Meta(MetaId),
    /// The error type: unifies with everything, silencing cascades after
    /// a reported error.
    Error,
}

/// One parameter of a function type: the type plus an optional call
/// label. Labels are advisory metadata — declaration parameter names and
/// record field names ride the type so call sites can name arguments —
/// and unification NEVER compares them.
#[derive(Clone, Debug)]
pub struct TParam {
    pub label: Option<String>,
    pub ty: Type,
}

/// An unlabeled function type (the common construction).
pub fn t_fun(args: Vec<Type>, ret: Type) -> Type {
    Type::Fun(
        args.into_iter()
            .map(|ty| TParam { label: None, ty })
            .collect(),
        Box::new(ret),
    )
}

#[derive(Clone, Debug)]
pub struct Scheme {
    pub vars: Vec<TyVar>,
    pub ty: Type,
}

pub fn mono_scheme(ty: Type) -> Scheme {
    Scheme {
        vars: Vec::new(),
        ty,
    }
}

// Builtins ---------------------------------------------------------------------

pub fn tc_int() -> TyCon {
    TyCon {
        name: b::ty_int(),
        arity: 0,
    }
}

pub fn tc_float() -> TyCon {
    TyCon {
        name: b::ty_float(),
        arity: 0,
    }
}

pub fn tc_string() -> TyCon {
    TyCon {
        name: b::ty_string(),
        arity: 0,
    }
}

pub fn tc_bool() -> TyCon {
    TyCon {
        name: b::ty_bool(),
        arity: 0,
    }
}

pub fn tc_list() -> TyCon {
    TyCon {
        name: b::ty_list(),
        arity: 1,
    }
}

pub fn tc_app() -> TyCon {
    TyCon {
        name: b::ty_app(),
        arity: 0,
    }
}

pub fn t_int() -> Type {
    Type::Con(tc_int(), Vec::new())
}

pub fn t_float() -> Type {
    Type::Con(tc_float(), Vec::new())
}

pub fn t_string() -> Type {
    Type::Con(tc_string(), Vec::new())
}

pub fn t_bool() -> Type {
    Type::Con(tc_bool(), Vec::new())
}

pub fn t_app() -> Type {
    Type::Con(tc_app(), Vec::new())
}

pub fn t_list(a: Type) -> Type {
    Type::Con(tc_list(), vec![a])
}

// Rendering ----------------------------------------------------------------------

struct Namer {
    assigned: BTreeMap<Unique, String>,
    n_star: usize,
    n_num: usize,
}

/// Render several types with one consistent naming of leftover
/// metavariables (`a`, `b`, … for Star; `number`, `number2`, … for Num) —
/// one diagnostic always names types coherently.
pub fn render_types(arena: &mut MetaArena, ts: &[Type]) -> Vec<String> {
    let zonked: Vec<Type> = ts.iter().map(|t| arena.zonk(t)).collect();
    let mut namer = Namer {
        assigned: BTreeMap::new(),
        n_star: 0,
        n_num: 0,
    };
    zonked
        .iter()
        .map(|t| render(arena, &mut namer, t))
        .collect()
}

fn render(arena: &MetaArena, namer: &mut Namer, ty: &Type) -> String {
    match ty {
        Type::Con(c, args) => {
            if args.is_empty() {
                c.name.text.clone()
            } else {
                let args2: Vec<String> = args.iter().map(|a| render(arena, namer, a)).collect();
                format!("{}<{}>", c.name.text, args2.join(", "))
            }
        }
        Type::Fun(args, ret) => {
            let args2: Vec<String> = args
                .iter()
                .map(|p| {
                    let t = render(arena, namer, &p.ty);
                    match &p.label {
                        Some(l) => format!("{l}: {t}"),
                        None => t,
                    }
                })
                .collect();
            format!("({}) -> {}", args2.join(", "), render(arena, namer, ret))
        }
        Type::Var(v) => v.text.clone(),
        Type::Error => "?".to_string(),
        Type::Meta(m) => {
            let cell = arena.cell(*m);
            if let Some(t) = namer.assigned.get(&cell.unique) {
                return t.clone();
            }
            let t = match cell.flavor {
                Flavor::Star => {
                    let t = star_name(namer.n_star);
                    namer.n_star += 1;
                    t
                }
                Flavor::Num => {
                    let t = num_name(namer.n_num);
                    namer.n_num += 1;
                    t
                }
            };
            namer.assigned.insert(cell.unique, t.clone());
            t
        }
    }
}

fn star_name(n: usize) -> String {
    let letter = char::from(b'a' + (n % 26) as u8);
    if n < 26 {
        letter.to_string()
    } else {
        format!("{}{}", letter, n / 26)
    }
}

fn num_name(n: usize) -> String {
    if n == 0 {
        "number".to_string()
    } else {
        format!("number{}", n + 1)
    }
}

pub fn render_type(arena: &mut MetaArena, t: &Type) -> String {
    render_types(arena, std::slice::from_ref(t))
        .pop()
        .unwrap_or_else(|| "?".to_string())
}

pub fn render_scheme(arena: &mut MetaArena, s: &Scheme) -> String {
    render_type(arena, &s.ty)
}
