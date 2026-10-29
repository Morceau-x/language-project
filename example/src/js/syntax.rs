//! A minimal JavaScript AST and its printer. Readability is a stated
//! requirement of the backend: 2-space indent, no IIFEs, precedence-aware
//! (slightly conservative) parenthesisation, and a line-width budget —
//! expressions render flat while they fit in `LINE_WIDTH` columns and
//! break structurally (objects one field per line, calls one argument per
//! line) once they don't, so nested data literals read like hand-written
//! JS instead of a wall.

use crate::maxim_panic;

#[derive(Clone, Debug)]
pub struct JsModule {
    /// (alias, path): `import * as alias from "path";`
    pub imports: Vec<(String, String)>,
    pub decls: Vec<JsDecl>,
}

#[derive(Clone, Debug)]
pub enum JsDecl {
    ExportFn(String, Vec<String>, Vec<JsStmt>),
    ExportConst(String, JsExpr),
    /// `export let name;` followed by initialisation statements (used
    /// when a top-level value needs statements — never an IIFE).
    ExportLetInit(String, Vec<JsStmt>),
}

#[derive(Clone, Debug)]
pub enum JsStmt {
    Const(String, JsExpr),
    Let(String),
    Assign(String, JsExpr),
    Expr(JsExpr),
    Return(JsExpr),
    If(JsExpr, Vec<JsStmt>, Vec<JsStmt>),
    Switch(JsExpr, Vec<(JsExpr, Vec<JsStmt>)>, Option<Vec<JsStmt>>),
    Break,
    Throw(String),
}

#[derive(Clone, Debug)]
pub enum ArrowBody {
    Expr(Box<JsExpr>),
    Stmts(Vec<JsStmt>),
}

#[derive(Clone, Debug)]
pub enum JsExpr {
    Num(String),
    Str(String),
    Bool(bool),
    Var(String),
    Arrow(Vec<String>, ArrowBody),
    Call(Box<JsExpr>, Vec<JsExpr>),
    Obj(Vec<(String, JsExpr)>),
    /// `{ ...base, f: v }` — record update.
    ObjSpread(Box<JsExpr>, Vec<(String, JsExpr)>),
    Field(Box<JsExpr>, String),
    Bin(&'static str, Box<JsExpr>, Box<JsExpr>),
    Un(&'static str, Box<JsExpr>),
    Cond(Box<JsExpr>, Box<JsExpr>, Box<JsExpr>),
}

// Rendering ---------------------------------------------------------------------

/// An expression fits on the current line up to here; past it, the
/// breakable constructs (objects, calls, arrows) go multi-line. Atoms
/// longer than the budget still render whole — there is nothing to break.
const LINE_WIDTH: usize = 100;

/// Code-point length, matching the Haskell `T.length` column math.
fn len(s: &str) -> usize {
    s.chars().count()
}

pub fn render_js_module(m: &JsModule) -> String {
    let mut lines: Vec<String> = m
        .imports
        .iter()
        .map(|(alias, path)| format!("import * as {alias} from {};", quote(path)))
        .collect();
    lines.push(String::new());
    for d in &m.decls {
        lines.extend(render_decl(d));
        lines.push(String::new());
    }
    // T.unlines: every line gets a trailing newline.
    let mut out = String::new();
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

fn render_decl(d: &JsDecl) -> Vec<String> {
    match d {
        JsDecl::ExportFn(name, params, body) => {
            let mut out = vec![format!(
                "export function {name}({}) {{",
                params.join(", ")
            )];
            for s in body {
                out.extend(stmt(1, s));
            }
            out.push("}".to_string());
            out
        }
        JsDecl::ExportConst(name, e) => expr_line(0, &format!("export const {name} = "), ";", e),
        JsDecl::ExportLetInit(name, body) => {
            let mut out = vec![format!("export let {name};")];
            for s in body {
                out.extend(stmt(0, s));
            }
            out
        }
    }
}

fn ind(d: usize) -> String {
    " ".repeat(2 * d)
}

/// Statements come out fully indented for block depth `d`.
fn stmt(d: usize, s: &JsStmt) -> Vec<String> {
    match s {
        JsStmt::Const(n, e) => expr_line(d, &format!("const {n} = "), ";", e),
        JsStmt::Let(n) => vec![format!("{}let {n};", ind(d))],
        JsStmt::Assign(n, e) => expr_line(d, &format!("{n} = "), ";", e),
        JsStmt::Expr(e) => expr_line(d, "", ";", e),
        JsStmt::Return(e) => expr_line(d, "return ", ";", e),
        JsStmt::If(c, t, e) => {
            let mut out = expr_line(d, "if (", ") {", c);
            for s2 in t {
                out.extend(stmt(d + 1, s2));
            }
            if e.is_empty() {
                out.push(format!("{}}}", ind(d)));
            } else {
                out.push(format!("{}}} else {{", ind(d)));
                for s2 in e {
                    out.extend(stmt(d + 1, s2));
                }
                out.push(format!("{}}}", ind(d)));
            }
            out
        }
        JsStmt::Switch(scrut, cases, deflt) => {
            let mut out = expr_line(d, "switch (", ") {", scrut);
            for (lbl, body) in cases {
                out.push(format!("{}case {}: {{", ind(d + 1), expr_flat(0, lbl)));
                for s2 in body {
                    out.extend(stmt(d + 2, s2));
                }
                out.push(format!("{}}}", ind(d + 1)));
            }
            if let Some(body) = deflt {
                out.push(format!("{}default: {{", ind(d + 1)));
                for s2 in body {
                    out.extend(stmt(d + 2, s2));
                }
                out.push(format!("{}}}", ind(d + 1)));
            }
            out.push(format!("{}}}", ind(d)));
            out
        }
        JsStmt::Break => vec![format!("{}break;", ind(d))],
        JsStmt::Throw(msg) => vec![format!("{}throw new Error({});", ind(d), quote(msg))],
    }
}

/// One statement line built around an expression: `prefix` (after the
/// indent) opens it, `suffix` closes whatever line the expression ends on.
fn expr_line(d: usize, prefix: &str, suffix: &str, e: &JsExpr) -> Vec<String> {
    let opening = format!("{}{}", ind(d), prefix);
    let (h, ls) = render(d, len(&opening), 0, e);
    let mut out = vec![format!("{opening}{h}")];
    out.extend(ls);
    on_last(&mut out, |l| l.push_str(suffix));
    out
}

/// A rendered fragment: text continuing the caller's current line, plus
/// fully-indented follow-on lines.
type Fragment = (String, Vec<String>);

fn on_last(xs: &mut [String], f: impl FnOnce(&mut String)) {
    if let Some(last) = xs.last_mut() {
        f(last);
    }
}

/// Append to the fragment's final line.
fn frag_last(s: &str, mut frag: Fragment) -> Fragment {
    if frag.1.is_empty() {
        frag.0.push_str(s);
    } else {
        on_last(&mut frag.1, |l| l.push_str(s));
    }
    frag
}

/// Join two fragments with a separator on the shared line.
fn glue(f1: Fragment, sep: &str, f2: Fragment) -> Fragment {
    let (h1, t1) = f1;
    let (h2, t2) = f2;
    if t1.is_empty() {
        (format!("{h1}{sep}{h2}"), t2)
    } else {
        let mut t = t1;
        on_last(&mut t, |l| {
            l.push_str(sep);
            l.push_str(&h2);
        });
        t.extend(t2);
        (h1, t)
    }
}

/// The column the fragment's last line ends on, given the column its
/// first character starts at.
fn end_col(col: usize, frag: &Fragment) -> usize {
    match frag.1.last() {
        None => col + len(&frag.0),
        Some(last) => len(last),
    }
}

/// `render(d, col, ctx, e)`: render for a line already holding `col`
/// characters at block depth `d`, in precedence context `ctx`. Flat while
/// it fits and nothing inside forces statements.
fn render(depth: usize, col: usize, ctx: u8, e: &JsExpr) -> Fragment {
    // `expr_flat` is only computed when nothing inside forces statements
    // (matching the Haskell's laziness: a statement-bodied arrow has no
    // flat form at all).
    if !forces_break(e) {
        let flat = expr_flat(ctx, e);
        if col + len(&flat) <= LINE_WIDTH {
            return (flat, Vec::new());
        }
    }
    let par = |frag: Fragment| -> Fragment {
        if prec(e) < ctx {
            frag_last(")", glue(("(".to_string(), Vec::new()), "", frag))
        } else {
            frag
        }
    };
    match e {
        JsExpr::Obj(fields) => par(fields_block(depth, None, fields)),
        JsExpr::ObjSpread(base, fields) => par(fields_block(depth, Some(base), fields)),
        JsExpr::Call(f, args) => par(call_block(depth, col, f, args)),
        JsExpr::Arrow(params, body) => par(arrow_block(depth, col, params, body)),
        JsExpr::Field(b, f) => par(frag_last(&format!(".{f}"), render(depth, col, 17, b))),
        JsExpr::Un(op, x) => par(glue(
            (op.to_string(), Vec::new()),
            "",
            render(depth, col + len(op), 16, x),
        )),
        JsExpr::Bin(op, l, r) => {
            let (lctx, rctx) = bin_ctx(op);
            let lf = render(depth, col, lctx, l);
            let sep = format!(" {op} ");
            let rf = render(depth, end_col(col, &lf) + len(&sep), rctx, r);
            par(glue(lf, &sep, rf))
        }
        JsExpr::Cond(c, t, f) => {
            let cf = render(depth, col, 4, c);
            let tf = render(depth, end_col(col, &cf) + 3, 4, t);
            let ctf = glue(cf, " ? ", tf);
            let ff = render(depth, end_col(col, &ctf) + 3, 3, f);
            par(glue(ctf, " : ", ff))
        }
        // Atoms have no broken form; an over-budget one renders whole.
        _ => (expr_flat(ctx, e), Vec::new()),
    }
}

/// One entry per line at depth+1, comma after every entry but the last,
/// closer at the current depth. No trailing comma — matching the Maxim
/// formatter's own multiline style.
fn fields_block(depth: usize, mbase: Option<&JsExpr>, fields: &[(String, JsExpr)]) -> Fragment {
    let mut entries: Vec<Vec<String>> = Vec::new();
    if let Some(b) = mbase {
        let pfx = format!("{}...", ind(depth + 1));
        let (h, ls) = render(depth + 1, len(&pfx), 2, b);
        let mut entry = vec![format!("{pfx}{h}")];
        entry.extend(ls);
        entries.push(entry);
    }
    for (k, v) in fields {
        let pfx = format!("{}{k}: ", ind(depth + 1));
        let (h, ls) = render(depth + 1, len(&pfx), 0, v);
        let mut entry = vec![format!("{pfx}{h}")];
        entry.extend(ls);
        entries.push(entry);
    }
    let mut lines = comma_sep(entries);
    lines.push(format!("{}}}", ind(depth)));
    ("{".to_string(), lines)
}

fn call_block(depth: usize, col: usize, f: &JsExpr, args: &[JsExpr]) -> Fragment {
    let (open_h, open_ls) = frag_last("(", render(depth, col, 17, f));
    let entries: Vec<Vec<String>> = args
        .iter()
        .map(|a| {
            let pfx = ind(depth + 1);
            let (h, ls) = render(depth + 1, len(&pfx), 0, a);
            let mut entry = vec![format!("{pfx}{h}")];
            entry.extend(ls);
            entry
        })
        .collect();
    let mut lines = open_ls;
    lines.extend(comma_sep(entries));
    lines.push(format!("{})", ind(depth)));
    (open_h, lines)
}

fn arrow_block(depth: usize, col: usize, params: &[String], body: &ArrowBody) -> Fragment {
    match body {
        ArrowBody::Stmts(stmts) => {
            let mut lines: Vec<String> = Vec::new();
            for s in stmts {
                lines.extend(stmt(depth + 1, s));
            }
            lines.push(format!("{}}}", ind(depth)));
            (format!("({}) => {{", params.join(", ")), lines)
        }
        ArrowBody::Expr(b) => {
            let pfx = format!("({}) => ", params.join(", "));
            match **b {
                JsExpr::Obj(_) => frag_last(
                    ")",
                    glue(
                        (format!("{pfx}("), Vec::new()),
                        "",
                        render(depth, col + len(&pfx) + 1, 0, b),
                    ),
                ),
                _ => glue(
                    (pfx.clone(), Vec::new()),
                    "",
                    render(depth, col + len(&pfx), 2, b),
                ),
            }
        }
    }
}

fn comma_sep(entries: Vec<Vec<String>>) -> Vec<String> {
    let n = entries.len();
    let mut out = Vec::new();
    for (i, mut entry) in entries.into_iter().enumerate() {
        if i + 1 < n {
            on_last(&mut entry, |l| l.push(','));
        }
        out.extend(entry);
    }
    out
}

/// True when the expression can only render multi-line: a
/// statement-bodied arrow somewhere inside.
fn forces_break(e: &JsExpr) -> bool {
    match e {
        JsExpr::Arrow(_, ArrowBody::Stmts(_)) => true,
        JsExpr::Arrow(_, ArrowBody::Expr(b)) => forces_break(b),
        JsExpr::Call(f, args) => forces_break(f) || args.iter().any(forces_break),
        JsExpr::Obj(fields) => fields.iter().any(|(_, v)| forces_break(v)),
        JsExpr::ObjSpread(base, fields) => {
            forces_break(base) || fields.iter().any(|(_, v)| forces_break(v))
        }
        JsExpr::Field(b, _) => forces_break(b),
        JsExpr::Bin(_, l, r) => forces_break(l) || forces_break(r),
        JsExpr::Un(_, x) => forces_break(x),
        JsExpr::Cond(c, t, f) => forces_break(c) || forces_break(t) || forces_break(f),
        JsExpr::Num(_) | JsExpr::Str(_) | JsExpr::Bool(_) | JsExpr::Var(_) => false,
    }
}

/// `expr_flat(ctx, e)`: the one-line rendering, with parens when `e`
/// binds looser than the context. Callers guard statement-bodied arrows
/// with `forces_break`.
fn expr_flat(ctx: u8, e: &JsExpr) -> String {
    let rendered = match e {
        JsExpr::Num(t) => t.clone(),
        JsExpr::Str(t) => quote(t),
        JsExpr::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        JsExpr::Var(t) => t.clone(),
        JsExpr::Arrow(params, body) => {
            let head = format!("({}) => ", params.join(", "));
            match body {
                ArrowBody::Expr(be) => match **be {
                    JsExpr::Obj(_) => format!("{head}({})", expr_flat(0, be)),
                    _ => format!("{head}{}", expr_flat(2, be)),
                },
                ArrowBody::Stmts(_) => {
                    maxim_panic!("expr_flat: statement-bodied arrow (guarded by forces_break)")
                }
            }
        }
        JsExpr::Call(f, args) => format!(
            "{}({})",
            expr_flat(17, f),
            args.iter()
                .map(|a| expr_flat(0, a))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        JsExpr::Obj(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", expr_flat(0, v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        JsExpr::ObjSpread(base, fields) => {
            let mut parts = vec![format!("...{}", expr_flat(2, base))];
            parts.extend(
                fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", expr_flat(0, v))),
            );
            format!("{{ {} }}", parts.join(", "))
        }
        JsExpr::Field(b, f) => format!("{}.{f}", expr_flat(17, b)),
        JsExpr::Bin(op, l, r) => {
            let (lctx, rctx) = bin_ctx(op);
            format!("{} {op} {}", expr_flat(lctx, l), expr_flat(rctx, r))
        }
        JsExpr::Un(op, x) => format!("{op}{}", expr_flat(16, x)),
        JsExpr::Cond(c, t, f) => format!(
            "{} ? {} : {}",
            expr_flat(4, c),
            expr_flat(4, t),
            expr_flat(3, f)
        ),
    };
    if prec(e) < ctx {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn prec(e: &JsExpr) -> u8 {
    match e {
        JsExpr::Num(_)
        | JsExpr::Str(_)
        | JsExpr::Bool(_)
        | JsExpr::Var(_)
        | JsExpr::Obj(_)
        | JsExpr::ObjSpread(_, _) => 20,
        JsExpr::Call(_, _) | JsExpr::Field(_, _) => 17,
        JsExpr::Un(_, _) => 15,
        JsExpr::Bin(op, _, _) => bin_prec(op),
        JsExpr::Cond(_, _, _) => 3,
        JsExpr::Arrow(_, _) => 2,
    }
}

fn bin_prec(op: &str) -> u8 {
    match op {
        // Same tier as unary: see bin_ctx — '**' takes flipped child
        // contexts.
        "**" => 15,
        "*" | "/" | "%" => 14,
        "+" | "-" => 13,
        "<" | ">" | "<=" | ">=" => 11,
        "&&" => 5,
        "||" => 4,
        _ => 10,
    }
}

/// Precedence contexts for a binary node's children. Left-associative
/// operators keep equal-precedence children flat on the left and
/// parenthesize them on the right. '**' flips both: JS makes it
/// right-associative AND rejects an unparenthesized unary or '**'
/// expression as its LEFT operand (a SyntaxError, not a precedence
/// matter), so the left child needs the strictly-higher context.
fn bin_ctx(op: &str) -> (u8, u8) {
    if op == "**" {
        (bin_prec(op) + 1, bin_prec(op))
    } else {
        (bin_prec(op), bin_prec(op) + 1)
    }
}

fn quote(t: &str) -> String {
    let mut out = String::with_capacity(t.len() + 2);
    out.push('"');
    for c in t.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    //! Ported from `compiler/test/JsSyntaxTests.hs`. The width-aware
    //! printer: flat while it fits, structural breaks past the budget,
    //! deterministic, no trailing commas. The budget counts the line up
    //! to the end of the expression; a 1-character statement suffix (`;`
    //! or `,`) may overhang — that tolerance is deliberate and pinned
    //! here.

    use super::*;

    fn decl_lines(d: JsDecl) -> Vec<String> {
        render_js_module(&JsModule {
            imports: Vec::new(),
            decls: vec![d],
        })
        .lines()
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty())
        .map(String::from)
        .collect()
    }

    // export const cfg = { a: "<k times x>" };
    fn const_lines(k: usize) -> Vec<String> {
        decl_lines(JsDecl::ExportConst(
            "cfg".to_string(),
            JsExpr::Obj(vec![("a".to_string(), JsExpr::Str("x".repeat(k)))]),
        ))
    }

    fn long_str() -> JsExpr {
        JsExpr::Str("x".repeat(60))
    }

    #[test]
    fn an_expression_at_the_width_budget_stays_flat() {
        match &const_lines(72)[..] {
            [l] => assert_eq!(l.chars().count(), 101), // 100 through the closing brace, +1 for ;
            ls => panic!("expected one line, got {}", ls.len()),
        }
    }

    #[test]
    fn one_past_the_budget_breaks_one_field_per_line() {
        assert_eq!(
            const_lines(73),
            vec![
                "export const cfg = {".to_string(),
                format!("  a: \"{}\"", "x".repeat(73)),
                "};".to_string(),
            ]
        );
    }

    #[test]
    fn broken_objects_comma_separate_all_fields_but_the_last() {
        assert_eq!(
            decl_lines(JsDecl::ExportConst(
                "pair".to_string(),
                JsExpr::Obj(vec![
                    ("first".to_string(), long_str()),
                    ("second".to_string(), long_str()),
                ]),
            )),
            vec![
                "export const pair = {".to_string(),
                format!("  first: \"{}\",", "x".repeat(60)),
                format!("  second: \"{}\"", "x".repeat(60)),
                "};".to_string(),
            ]
        );
    }

    #[test]
    fn a_statement_bodied_arrow_breaks_and_indents_relative_to_its_line() {
        assert_eq!(
            decl_lines(JsDecl::ExportConst(
                "f".to_string(),
                JsExpr::Arrow(
                    vec!["x".to_string()],
                    ArrowBody::Stmts(vec![JsStmt::Return(JsExpr::Var("x".to_string()))]),
                ),
            )),
            vec![
                "export const f = (x) => {".to_string(),
                "  return x;".to_string(),
                "};".to_string(),
            ]
        );
    }

    #[test]
    fn rendering_is_deterministic() {
        let m = JsModule {
            imports: Vec::new(),
            decls: vec![JsDecl::ExportConst(
                "pair".to_string(),
                JsExpr::Obj(vec![
                    ("first".to_string(), long_str()),
                    ("second".to_string(), long_str()),
                ]),
            )],
        };
        assert_eq!(render_js_module(&m), render_js_module(&m));
    }
}
