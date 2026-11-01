//! The compilation driver: discover the module graph from an entry file,
//! compile the stdlib and the user modules in dependency order, and emit
//! ES modules. Cross-module knowledge flows ONLY through `ModuleExports`
//! (names) and `Iface` (types) — the incremental-compilation seam.
//!
//! A module that fails poisons only its dependents (reported as a short
//! note, not an error cascade); independent subgraphs still compile.
//!
//! File loading goes through an injected [`Loader`] so a language server
//! can overlay dirty buffers over the disk.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ast as a;
use crate::builtin::{builtin_ctors, builtin_modules, builtin_types};
use crate::core::lower::{JsxApi, lower_to_core};
use crate::cst::lower::lower_module;
use crate::diag::{Diagnostic, Severity};
use crate::index::name_occurrences;
use crate::intrinsics::base_tc_env;
use crate::js::emit::emit_module;
use crate::match_analysis::{DTree, analyze_match, collect_matches};
use crate::names::{ModuleExports, ModuleName, Name, RawName};
use crate::parser::parse_module_recovering;
use crate::rename::{RenameEnv, rename_module};
use crate::source::SourceFile;
use crate::span::{FileId, START_POS, Span};
use crate::types::env::{TcEnv, merge_iface};
use crate::types::infer::check_module;
use crate::types::ty::Type;

#[derive(Clone, Debug)]
pub struct CompileOptions {
    pub entry: PathBuf,
    pub out_dir: PathBuf,
    pub stdlib_dir: PathBuf,
    /// Type-check only: no code generation, nothing written.
    pub check_only: bool,
}

/// File loading, injected so tooling can overlay dirty buffers.
pub trait Loader {
    fn load(&mut self, path: &Path) -> Option<String>;
}

/// The default loader: straight off the disk.
pub struct DiskLoader;

impl Loader for DiskLoader {
    fn load(&mut self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

#[derive(Debug)]
pub struct BuildOutcome {
    /// Everything loaded, for diagnostic rendering.
    pub sources: Vec<SourceFile>,
    pub diags: Vec<Diagnostic>,
    /// No error-severity diagnostics (warnings allowed).
    pub succeeded: bool,
    pub entry_module: Option<ModuleName>,
    pub entry_has_main: bool,
    /// Successfully renamed modules, each with the TYPE names every one
    /// of its imports exports (a linter needs this: an import used only
    /// in an annotation is still used).
    pub renamed_modules: Vec<(a::Module<Name>, BTreeMap<ModuleName, Vec<String>>)>,
    /// The type of every expression in every checked module, by span —
    /// a language server's hover data.
    pub expr_types: BTreeMap<Span, Type>,
    /// Every name occurrence in every checked module — go-to-definition
    /// resolves these via `Name::def_span`.
    pub occurrences: Vec<(Span, Name)>,
}

/// The prelude providers, always compiled first in this order: their
/// constructors and types feed the unqualified prelude (`Some`, `Ok`, …),
/// so every later stdlib module can use them without an import. `Html`
/// is here for its TYPES — JSX elements mean `Html<msg>` in any module,
/// so the name must resolve without an import. It compiles before `List`
/// and must stay self-contained (local recursion only).
const PRELUDE_MODULE_NAMES: [&str; 3] = ["Option", "Result", "Html"];

/// Stdlib membership is the directory listing — dropping a new `.mx`
/// into the stdlib is all it takes. Prelude providers first, the rest
/// alphabetically. A missing directory falls back to the prelude list so
/// the per-module E5002 path reports it.
fn discover_stdlib_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return PRELUDE_MODULE_NAMES.iter().map(|s| s.to_string()).collect();
    };
    let mut mods: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension().is_some_and(|x| x == "mx"))
                .then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    mods.sort();
    let mut out: Vec<String> = PRELUDE_MODULE_NAMES.iter().map(|s| s.to_string()).collect();
    out.extend(
        mods.into_iter()
            .filter(|m| !PRELUDE_MODULE_NAMES.contains(&m.as_str())),
    );
    out
}

const RUNTIME_FILE_NAME: &str = "maxim_runtime.mjs";

/// One loaded-and-parsed module, pre-rename.
struct Loaded {
    name: ModuleName,
    ast: a::Module<RawName>,
    imports: Vec<(ModuleName, Span)>,
    /// Error recovery salvaged this module around broken declarations:
    /// its healthy declarations still rename and type-check, but it must
    /// not emit JS, and dependents are poisoned as if the parse had
    /// failed outright.
    parse_failed: bool,
}

fn no_span() -> Span {
    Span::new(FileId(-2), START_POS, START_POS)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn build_entry(load: &mut dyn Loader, opts: &CompileOptions) -> BuildOutcome {
    let mut sources: Vec<SourceFile> = Vec::new();
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut next_fid: i32 = 0;

    let mut load_and_parse = |sources: &mut Vec<SourceFile>,
                              diags: &mut Vec<Diagnostic>,
                              next_fid: &mut i32,
                              load: &mut dyn Loader,
                              path: &Path|
     -> Result<Loaded, String> {
        let Some(src) = load.load(path) else {
            return Err(format!("cannot read {}", path.display()));
        };
        let fid = FileId(*next_fid);
        *next_fid += 1;
        sources.push(SourceFile {
            id: fid,
            path: path.display().to_string(),
            text: src.clone(),
        });
        match parse_module_recovering(fid, &src) {
            Err(d) => {
                diags.push(d);
                Err("parse failed".to_string())
            }
            Ok((cst, parse_diags)) => {
                let parse_failed = !parse_diags.is_empty();
                diags.extend(parse_diags);
                match lower_module(&cst) {
                    Err(d) => {
                        diags.push(d);
                        Err("parse failed".to_string())
                    }
                    Ok(ast) => {
                        let name = ast.name.clone();
                        if name.0 != file_stem(path) {
                            diags.push(
                                Diagnostic::error(
                                    "E5001",
                                    "MODULE NAME MISMATCH",
                                    ast.name_span,
                                    format!(
                                        "the file is named `{}.mx` but the module calls itself `{}`",
                                        file_stem(path),
                                        name.0
                                    ),
                                )
                                .with_hints(vec![
                                    "Rename the file or the module so they agree.".to_string(),
                                ]),
                            );
                        }
                        let imports = ast
                            .imports
                            .iter()
                            .map(|i| (i.module.clone(), i.span))
                            .collect();
                        Ok(Loaded {
                            name,
                            ast,
                            imports,
                            parse_failed,
                        })
                    }
                }
            }
        }
    };

    // 1. The stdlib: prelude providers first, then the rest of the
    // directory (no explicit imports between stdlib modules; the prelude
    // carries the cross-module names).
    let stdlib_names = discover_stdlib_names(&opts.stdlib_dir);
    let mut stdlib_ok: Vec<Loaded> = Vec::new();
    for name in &stdlib_names {
        let path = opts.stdlib_dir.join(format!("{name}.mx"));
        match load_and_parse(&mut sources, &mut diags, &mut next_fid, load, &path) {
            Err(msg) => {
                diags.push(
                    Diagnostic::error(
                        "E5002",
                        "STDLIB NOT FOUND",
                        no_span(),
                        format!("stdlib module `{name}`: {msg}"),
                    )
                    .with_hints(vec![
                        "Point --stdlib (or MAXIM_STDLIB) at the stdlib/core directory."
                            .to_string(),
                    ]),
                );
            }
            Ok(l) => stdlib_ok.push(l),
        }
    }

    // 2. The user module graph from the entry file.
    let source_root = opts
        .entry
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stdlib_set: BTreeSet<ModuleName> =
        stdlib_names.iter().map(|n| ModuleName(n.clone())).collect();
    let virtual_set: BTreeSet<ModuleName> = ["Debug", "String", "Int"]
        .iter()
        .map(|n| ModuleName(n.to_string()))
        .collect();
    let mut user_modules: BTreeMap<ModuleName, Loaded> = BTreeMap::new();

    // Recursive graph discovery, worklist style (each discovered module
    // queues its unseen imports; the Haskell recursion visits the same
    // set).
    fn discover(
        sources: &mut Vec<SourceFile>,
        diags: &mut Vec<Diagnostic>,
        next_fid: &mut i32,
        load: &mut dyn Loader,
        load_and_parse: &mut dyn FnMut(
            &mut Vec<SourceFile>,
            &mut Vec<Diagnostic>,
            &mut i32,
            &mut dyn Loader,
            &Path,
        ) -> Result<Loaded, String>,
        user_modules: &mut BTreeMap<ModuleName, Loaded>,
        stdlib_set: &BTreeSet<ModuleName>,
        virtual_set: &BTreeSet<ModuleName>,
        source_root: &Path,
        path: &Path,
    ) -> Option<ModuleName> {
        let l = load_and_parse(sources, diags, next_fid, load, path).ok()?;
        if stdlib_set.contains(&l.name) {
            diags.push(Diagnostic::error(
                "E5005",
                "RESERVED MODULE NAME",
                l.ast.name_span,
                format!("`{}` is a stdlib module name", l.name.0),
            ));
        }
        let name = l.name.clone();
        let imports = l.imports.clone();
        user_modules.insert(name.clone(), l);
        for (m, sp) in imports {
            if user_modules.contains_key(&m) || stdlib_set.contains(&m) || virtual_set.contains(&m)
            {
                continue;
            }
            let dep_path = source_root.join(format!("{}.mx", m.0));
            if dep_path.is_file() {
                discover(
                    sources,
                    diags,
                    next_fid,
                    load,
                    load_and_parse,
                    user_modules,
                    stdlib_set,
                    virtual_set,
                    source_root,
                    &dep_path,
                );
            } else {
                diags.push(Diagnostic::error(
                    "E5002",
                    "MODULE NOT FOUND",
                    sp,
                    format!(
                        "I can't find `{}` — I looked for {}",
                        m.0,
                        dep_path.display()
                    ),
                ));
            }
        }
        Some(name)
    }

    // A stdlib file given as the entry (e.g. `maxim lint
    // stdlib/core/List.mx`) is already loaded; re-discovering it would
    // trip the reserved-name check.
    let entry_canon = std::fs::canonicalize(&opts.entry).unwrap_or_else(|_| opts.entry.clone());
    let stdlib_entry = stdlib_names.iter().find(|n| {
        std::fs::canonicalize(opts.stdlib_dir.join(format!("{n}.mx")))
            .is_ok_and(|p| p == entry_canon)
    });
    let entry_name: Option<ModuleName> = match stdlib_entry {
        Some(n) => Some(ModuleName(n.clone())),
        None => discover(
            &mut sources,
            &mut diags,
            &mut next_fid,
            load,
            &mut load_and_parse,
            &mut user_modules,
            &stdlib_set,
            &virtual_set,
            &source_root,
            &opts.entry,
        ),
    };

    // 3. Topological order with cycle detection.
    let order = topo_order(&mut diags, &user_modules);

    // 4. Compile: stdlib first, then users in order.
    let mut tc_env: TcEnv = base_tc_env();
    let mut exports_map: BTreeMap<ModuleName, ModuleExports> = BTreeMap::new();
    let mut failed: BTreeSet<ModuleName> = BTreeSet::new();
    let mut entry_has_main = false;
    let mut seed: i64 = 0;
    let mut renamed_modules: Vec<(a::Module<Name>, BTreeMap<ModuleName, Vec<String>>)> = Vec::new();
    let mut expr_types_all: BTreeMap<Span, Type> = BTreeMap::new();
    let mut occurrences: Vec<(Span, Name)> = Vec::new();
    if !opts.check_only {
        let _ = std::fs::create_dir_all(&opts.out_dir);
    }

    let mut compile_one = |l: &Loaded,
                           tc_env: &mut TcEnv,
                           exports_map: &mut BTreeMap<ModuleName, ModuleExports>,
                           failed: &mut BTreeSet<ModuleName>,
                           seed: &mut i64,
                           diags: &mut Vec<Diagnostic>,
                           entry_has_main: &mut bool| {
        let failed_deps: Vec<&ModuleName> = l
            .imports
            .iter()
            .map(|(m, _)| m)
            .filter(|m| failed.contains(*m))
            .collect();
        if !failed_deps.is_empty() {
            diags.push(Diagnostic::warning(
                "W5006",
                "MODULE SKIPPED",
                l.ast.name_span,
                format!(
                    "skipping `{}` because its import{} failed: {}",
                    l.name.0,
                    if failed_deps.len() > 1 { "s" } else { "" },
                    failed_deps
                        .iter()
                        .map(|m| m.0.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
            failed.insert(l.name.clone());
            return;
        }
        // Auto modules: compiled stdlib exports merged over the wired-in
        // virtual modules (compiled exports win — `Map.unionWith
        // unionExports stdlib builtin` is left-biased inside each merge).
        let mut auto_mods = builtin_modules();
        for m in &stdlib_set {
            if let Some(e) = exports_map.get(m) {
                let merged = match auto_mods.get(m) {
                    Some(builtin) => union_exports(e, builtin),
                    None => e.clone(),
                };
                auto_mods.insert(m.clone(), merged);
            }
        }
        let mut prelude_ctors = builtin_ctors();
        let mut prelude_types = builtin_types();
        for pm in PRELUDE_MODULE_NAMES {
            if let Some(e) = exports_map.get(&ModuleName(pm.to_string())) {
                for (k, v) in &e.ctors {
                    prelude_ctors.entry(k.clone()).or_insert_with(|| v.clone());
                }
                for (k, v) in &e.types {
                    prelude_types.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        let rn_env = RenameEnv {
            auto_modules: auto_mods,
            user_modules: exports_map
                .iter()
                .filter(|(k, _)| !stdlib_set.contains(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            prelude_ctors,
            prelude_types: prelude_types.clone(),
        };
        match rename_module(&rn_env, *seed, &l.ast) {
            Err(ds) => {
                diags.extend(ds);
                failed.insert(l.name.clone());
            }
            Ok((renamed, exports, seed2)) => {
                *seed = seed2;
                // Occurrences need only the renamed AST, so record them
                // now: go-to-definition keeps working even if this module
                // fails to type-check.
                let mut occs = name_occurrences(&renamed);
                occs.extend(std::mem::take(&mut occurrences));
                occurrences = occs;
                let import_types: BTreeMap<ModuleName, Vec<String>> = l
                    .imports
                    .iter()
                    .filter_map(|(m, _)| {
                        exports_map
                            .get(m)
                            .map(|e| (m.clone(), e.types.keys().cloned().collect()))
                    })
                    .collect();
                renamed_modules.push((renamed.clone(), import_types));
                // Imported modules' type names become annotatable.
                let mut imported_types: BTreeMap<String, Name> = BTreeMap::new();
                for (m, _) in &l.imports {
                    if let Some(e) = exports_map.get(m) {
                        for (k, v) in &e.types {
                            imported_types.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                }
                // JSX is pinned to the STDLIB Html: elements build its
                // type (checker) and call its functions (lowering),
                // shadows notwithstanding. None while the stdlib itself
                // compiles — its modules don't use JSX.
                let jsx_html_con = exports_map
                    .get(&ModuleName("Html".to_string()))
                    .and_then(|html| html.types.get("Html"))
                    .and_then(|hn| tc_env.ty_cons.get(&hn.unique))
                    .map(|hi| hi.con.clone());
                let jsx_api: Option<JsxApi> = (|| {
                    let html = exports_map.get(&ModuleName("Html".to_string()))?;
                    let list = exports_map.get(&ModuleName("List".to_string()))?;
                    let option = exports_map.get(&ModuleName("Option".to_string()))?;
                    Some(JsxApi {
                        node: html.values.get("node")?.clone(),
                        text: html.values.get("text")?.clone(),
                        attr: html.values.get("attr")?.clone(),
                        on: html.values.get("on")?.clone(),
                        on_text: html.values.get("onText")?.clone(),
                        on_bool: html.values.get("onBool")?.clone(),
                        append: list.values.get("append")?.clone(),
                        some: option.ctors.get("Some")?.clone(),
                        none: option.ctors.get("None")?.clone(),
                    })
                })();
                let mut tc_env_here = tc_env.clone();
                for (k, v) in imported_types {
                    tc_env_here.type_names.entry(k).or_insert(v);
                }
                for (k, v) in &prelude_types {
                    tc_env_here
                        .type_names
                        .entry(k.clone())
                        .or_insert_with(|| v.clone());
                }
                tc_env_here.jsx_html = jsx_html_con;
                let (tc_result, artifacts, _arena) = check_module(&tc_env_here, &renamed);
                // Best-effort hover types survive a type error too.
                for (sp, t) in &artifacts.expr_types {
                    expr_types_all.insert(*sp, t.clone());
                }
                match tc_result {
                    Err(ds) => {
                        diags.extend(ds);
                        failed.insert(l.name.clone());
                    }
                    Ok(iface) => {
                        let mut env_self = tc_env_here.clone();
                        merge_iface(&iface, &mut env_self);
                        let mut trees: BTreeMap<Span, DTree> = BTreeMap::new();
                        let mut match_diags: Vec<Diagnostic> = Vec::new();
                        for (sp, arms) in collect_matches(&renamed) {
                            let (ds, tree) = analyze_match(&env_self, sp, arms);
                            match_diags.extend(ds);
                            trees.insert(sp, tree);
                        }
                        let match_has_error =
                            match_diags.iter().any(|d| d.severity == Severity::Error);
                        diags.extend(match_diags);
                        // A statement-position expression of function type
                        // is almost always a missing-argument bug now that
                        // a short call quietly builds a partial
                        // application.
                        for dsp in discarded_functions(&artifacts.expr_types, &renamed) {
                            diags.push(
                                Diagnostic::warning(
                                    "W3001",
                                    "DISCARDED FUNCTION",
                                    dsp,
                                    "this expression is a function, and its result is thrown away",
                                )
                                .with_hints(vec![
                                    "Did you forget arguments? A call with too few builds a function instead of running one."
                                        .to_string(),
                                ]),
                            );
                        }
                        if l.parse_failed || match_has_error {
                            failed.insert(l.name.clone());
                        } else {
                            if Some(&l.name) == entry_name.as_ref() {
                                *entry_has_main = exports.values.contains_key("main");
                            }
                            if !opts.check_only {
                                let core = lower_to_core(
                                    &env_self,
                                    &trees,
                                    &artifacts.try_kinds,
                                    &artifacts.expr_types,
                                    &artifacts.cast_fns,
                                    &artifacts.instances,
                                    jsx_api.as_ref(),
                                    &renamed,
                                );
                                let js = emit_module(&env_self, &iface, &core);
                                let _ = std::fs::write(
                                    opts.out_dir.join(format!("{}.mjs", l.name.0)),
                                    js,
                                );
                            }
                            merge_iface(&iface, tc_env);
                            exports_map.insert(l.name.clone(), exports);
                        }
                    }
                }
            }
        }
    };

    for l in &stdlib_ok {
        compile_one(
            l,
            &mut tc_env,
            &mut exports_map,
            &mut failed,
            &mut seed,
            &mut diags,
            &mut entry_has_main,
        );
    }
    for m in &order {
        if let Some(l) = user_modules.get(m) {
            compile_one(
                l,
                &mut tc_env,
                &mut exports_map,
                &mut failed,
                &mut seed,
                &mut diags,
                &mut entry_has_main,
            );
        }
    }

    // 5. Runtime + entry wrapper.
    let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
    if !opts.check_only && !has_errors {
        if let Some(runtime) = load.load(&opts.stdlib_dir.join(RUNTIME_FILE_NAME)) {
            let _ = std::fs::write(opts.out_dir.join(RUNTIME_FILE_NAME), runtime);
        }
        // Wrapper file: "_main.mjs", NOT "main.mjs" — module names start
        // uppercase, and case-insensitive filesystems (macOS!) would let
        // a "main.mjs" wrapper clobber a "Main.mjs" module.
        if let Some(en) = &entry_name {
            if entry_has_main {
                let _ = std::fs::write(
                    opts.out_dir.join("_main.mjs"),
                    format!("import {{ main }} from \"./{}.mjs\";\nmain();\n", en.0),
                );
            }
        }
    }

    let succeeded = !diags.iter().any(|d| d.severity == Severity::Error);
    BuildOutcome {
        sources,
        diags,
        succeeded,
        entry_module: entry_name,
        entry_has_main,
        renamed_modules,
        expr_types: expr_types_all,
        occurrences,
    }
}

/// DFS topological sort over the user modules, reporting cycles with
/// their full path.
fn topo_order(diags: &mut Vec<Diagnostic>, mods: &BTreeMap<ModuleName, Loaded>) -> Vec<ModuleName> {
    // 1 = visiting, 2 = done.
    fn visit(
        diags: &mut Vec<Diagnostic>,
        mods: &BTreeMap<ModuleName, Loaded>,
        marks: &mut BTreeMap<ModuleName, u8>,
        acc: &mut Vec<ModuleName>,
        path: &mut Vec<ModuleName>,
        m: &ModuleName,
    ) {
        let Some(l) = mods.get(m) else { return };
        match marks.get(m) {
            Some(2) => {}
            Some(1) => {
                let mut cycle: Vec<&str> = path.iter().rev().map(|p| p.0.as_str()).collect();
                cycle.insert(0, m.0.as_str());
                // reverse (m : path) renders oldest-first ending at m.
                cycle.reverse();
                diags.push(Diagnostic::error(
                    "E5004",
                    "IMPORT CYCLE",
                    l.ast.name_span,
                    format!(
                        "modules import each other in a loop: {}",
                        cycle.join(" -> ")
                    ),
                ));
            }
            _ => {
                marks.insert(m.clone(), 1);
                path.push(m.clone());
                for (dep, _) in &l.imports {
                    visit(diags, mods, marks, acc, path, dep);
                }
                path.pop();
                marks.insert(m.clone(), 2);
                acc.push(m.clone());
            }
        }
    }
    let mut marks = BTreeMap::new();
    let mut acc = Vec::new();
    for m in mods.keys() {
        let mut path = Vec::new();
        visit(diags, mods, &mut marks, &mut acc, &mut path, m);
    }
    acc
}

/// Left-biased union of two export sets (`a` wins).
fn union_exports(a: &ModuleExports, b: &ModuleExports) -> ModuleExports {
    let mut out = a.clone();
    for (k, v) in &b.values {
        out.values.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &b.ctors {
        out.ctors.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &b.types {
        out.types.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &b.components {
        out.components.entry(k.clone()).or_insert_with(|| v.clone());
    }
    out
}

/// Statement-position expressions whose (zonked) type is a function: the
/// W3001 sites. Lives here, not in Infer — `check_module` treats any
/// recorded diagnostic as failure, and this is only ever a warning.
fn discarded_functions(tys: &BTreeMap<Span, Type>, m: &a::Module<Name>) -> Vec<Span> {
    use crate::ast::*;
    fn expr(tys: &BTreeMap<Span, Type>, e: &Expr<Name>, out: &mut Vec<Span>) {
        match e {
            Expr::Block(_, stmts, result) => {
                for s in stmts {
                    match s {
                        Stmt::Let(_, _, _, e2) => expr(tys, e2, out),
                        Stmt::Discard(e2) => {
                            if matches!(tys.get(&e2.span()), Some(Type::Fun(_, _))) {
                                out.push(e2.span());
                            }
                            expr(tys, e2, out);
                        }
                    }
                }
                expr(tys, result, out);
            }
            Expr::If(_, c, t, e2) => {
                expr(tys, c, out);
                expr(tys, t, out);
                expr(tys, e2, out);
            }
            Expr::Match(_, s, arms) => {
                expr(tys, s, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        expr(tys, g, out);
                    }
                    expr(tys, &arm.body, out);
                }
            }
            Expr::Lam(_, _, _, b) => expr(tys, b, out),
            Expr::Call(_, f, args) => {
                expr(tys, f, out);
                for arg in args {
                    expr(tys, arg.expr(), out);
                }
            }
            Expr::Pipe(_, _, l, r) | Expr::Bin(_, _, _, l, r) => {
                expr(tys, l, out);
                expr(tys, r, out);
            }
            Expr::Unary(_, _, e2) | Expr::Field(_, e2, _, _) | Expr::Try(_, _, e2) => {
                expr(tys, e2, out)
            }
            Expr::Update(_, _, _, b, fs) => {
                expr(tys, b, out);
                for (_, _, e2) in fs {
                    expr(tys, e2, out);
                }
            }
            Expr::List(_, es, sprd) => {
                for e2 in es {
                    expr(tys, e2, out);
                }
                if let Some(s) = sprd {
                    expr(tys, s, out);
                }
            }
            Expr::Interp(_, parts) => {
                for p in parts {
                    if let InterpPart::Expr(e2) = p {
                        expr(tys, e2, out);
                    }
                }
            }
            Expr::Jsx(_, el) => jsx_expr(tys, el, out),
            _ => {}
        }
    }
    fn jsx_expr(tys: &BTreeMap<Span, Type>, el: &JsxElem<Name>, out: &mut Vec<Span>) {
        for at in &el.attrs {
            match at {
                JsxAttr::Attr(_, _, v) => {
                    if let Some(e) = v {
                        expr(tys, e, out);
                    }
                }
                JsxAttr::On(_, _, e) => expr(tys, e, out),
            }
        }
        for c in &el.children {
            match c {
                JsxChild::Text(_, _) => {}
                JsxChild::Elem(el2) => jsx_expr(tys, el2, out),
                JsxChild::Splice(_, e) | JsxChild::Spread(_, e) | JsxChild::Hole(_, e) => {
                    expr(tys, e, out)
                }
            }
        }
    }
    let mut out = Vec::new();
    for d in &m.decls {
        match d {
            a::Decl::Fn(fnd) => expr(tys, &fnd.body, &mut out),
            a::Decl::Let(_, _, _, e) => expr(tys, e, &mut out),
            a::Decl::Cast(cd) => expr(tys, &cd.body, &mut out),
            a::Decl::Type(_) | a::Decl::Component(_) => {}
        }
    }
    out
}
