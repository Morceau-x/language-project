//! `check-grammar`: checks the Maxim grammar you write in
//! `mini-parser/spec/maxim.md`. Run it as `cargo check-grammar` (an alias
//! in `.cargo/config.toml`).
//!
//! It reads the spec's ` ```ebnf ` blocks, checks that the rules fit
//! together, then runs them on samples: lexical rules on token texts,
//! syntactic rules on types, patterns, expressions and whole programs.
//! The samples are cut into tokens by the reference Maxim compiler's
//! lexer, so the syntactic grammar can be checked before you write your
//! own lexer.

mod earley;
mod ebnf;
mod lexical;
mod ll1;
mod samples;
mod tokens;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use earley::{Bnf, grouping, recognize};
use ebnf::{ExprKind, Grammar};
use samples::SyntaxSamples;
use tokens::NoTokens;

const USAGE: &str = "\
usage: check-grammar [--spec FILE] [SECTION...]

Check the grammar in mini-parser/spec/maxim.md (or FILE).
SECTION is one or more of: lexical, types, patterns, expressions,
programs, ll1. Without one, everything is checked.";

/// The token kinds the syntactic grammar can use by name.
const TOKENS: &[&str] = &[
    "LOWER",
    "UPPER",
    "INT",
    "FLOAT",
    "STRING",
    "STRING_START",
    "STRING_MIDDLE",
    "STRING_END",
    "BOL_LPAREN",
];

/// The lexical rules the spec must define.
const LEXICAL_RULES: &[&str] = &[
    "WHITESPACE",
    "COMMENT",
    "KEYWORD",
    "LOWER",
    "UPPER",
    "INT",
    "FLOAT",
    "STRING",
    "STRING_START",
    "STRING_MIDDLE",
    "STRING_END",
    "PUNCTUATION",
];

/// The syntactic rules the samples start from.
const STARTS: &[&str] = &["Module", "Type", "Pattern", "Expr"];

/// Maxim's keywords and punctuation: what a quoted text can be in the
/// syntactic grammar.
const QUOTABLE: &[&str] = &[
    "module",
    "import",
    "type",
    "fn",
    "cast",
    "component",
    "let",
    "match",
    "if",
    "else",
    "_",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    "<",
    ">",
    ",",
    ":",
    "::",
    ".",
    "..",
    "=",
    "==",
    "!=",
    "<=",
    ">=",
    "|",
    "||",
    "|>",
    "&&",
    "!",
    "->",
    "=>",
    "?",
    "+",
    "++",
    "-",
    "*",
    "**",
    "/",
    "~/",
    "%",
];

const SECTIONS: &[&str] = &[
    "lexical",
    "types",
    "patterns",
    "expressions",
    "programs",
    "ll1",
];

fn main() -> ExitCode {
    let mut spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec/maxim.md");
    let mut sections = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--spec" => match args.next() {
                Some(path) => spec = PathBuf::from(path),
                None => return usage(),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            section if SECTIONS.contains(&section) => sections.push(arg),
            _ => return usage(),
        }
    }
    if sections.is_empty() {
        sections = SECTIONS.iter().map(|s| s.to_string()).collect();
    }

    let markdown = match std::fs::read_to_string(&spec) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: cannot read {}: {error}", spec.display());
            return ExitCode::FAILURE;
        }
    };
    let grammar = match ebnf::parse_spec(&markdown) {
        Ok(grammar) => grammar,
        Err(errors) => {
            println!("The EBNF in {} doesn't parse:", spec.display());
            for error in errors {
                println!("  ✗ {error}");
            }
            return ExitCode::FAILURE;
        }
    };

    let mut report = Report::default();
    check_rules(&grammar, &mut report);
    for section in &sections {
        match section.as_str() {
            "lexical" => check_lexical(&grammar, &mut report),
            "types" => check_syntax(&grammar, &samples::TYPES, &mut report),
            "patterns" => check_syntax(&grammar, &samples::PATTERNS, &mut report),
            "expressions" => check_syntax(&grammar, &samples::EXPRESSIONS, &mut report),
            "programs" => check_programs(&grammar, &mut report),
            "ll1" => report_ll1(&grammar, &mut report),
            _ => unreachable!(),
        }
    }
    println!();
    if report.failures == 0 {
        println!("All checks pass.");
        ExitCode::SUCCESS
    } else {
        println!(
            "{} problem{} to fix.",
            report.failures,
            if report.failures == 1 { "" } else { "s" }
        );
        ExitCode::FAILURE
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::FAILURE
}

#[derive(Default)]
struct Report {
    failures: usize,
}

impl Report {
    fn title(&self, title: &str) {
        println!("\n{title}");
    }

    fn ok(&self, msg: &str) {
        println!("  ✓ {msg}");
    }

    fn fail(&mut self, msg: &str) {
        self.failures += 1;
        println!("  ✗ {msg}");
    }

    fn warn(&self, msg: &str) {
        println!("  ! {msg}");
    }

    fn note(&self, msg: &str) {
        println!("    {msg}");
    }
}

/// `UPPER_CASE` names are lexical rules.
fn is_lexical(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Do the rules fit together?
fn check_rules(grammar: &Grammar, report: &mut Report) {
    report.title("Rules");
    let failures = report.failures;
    let mut defined: HashMap<&str, usize> = HashMap::new();
    for rule in &grammar.rules {
        if let Some(first) = defined.insert(&rule.name, rule.line) {
            report.fail(&format!(
                "line {}: `{}` is already defined, line {first}",
                rule.line, rule.name
            ));
        }
    }
    for rule in &grammar.rules {
        let lexical = is_lexical(&rule.name);
        let mut refs = Vec::new();
        rule.expr.refs(&mut refs);
        for (name, line) in refs {
            let is_token = TOKENS.contains(&name.as_str());
            if lexical && !is_lexical(&name) {
                report.fail(&format!(
                    "line {line}: `{}` is lexical (UPPER_CASE), so it can't use the syntactic rule `{name}`",
                    rule.name
                ));
            } else if !lexical && is_lexical(&name) && !is_token {
                report.fail(&format!(
                    "line {line}: `{}` uses `{name}`, but the syntactic grammar can only use whole tokens: {}",
                    rule.name,
                    TOKENS.join(", ")
                ));
            } else if !defined.contains_key(name.as_str()) && !(is_token && !lexical) {
                report.fail(&format!("line {line}: `{name}` is used but never defined"));
            }
        }
        if !lexical {
            rule.expr.walk(&mut |expr| match &expr.kind {
                ExprKind::Class(_) | ExprKind::Diff(..) => report.fail(&format!(
                    "line {}: `{}` is about characters: that belongs in the lexical grammar",
                    expr.line, expr.text
                )),
                ExprKind::Lit(text) if !QUOTABLE.contains(&text.as_str()) => report.fail(&format!(
                    "line {}: `{}` is not a Maxim keyword or punctuation",
                    expr.line, expr.text
                )),
                _ => {}
            });
        }
    }
    // Reachability: from the token rules, and from the start rules.
    let rules: HashMap<&str, &ebnf::Rule> =
        grammar.rules.iter().map(|r| (r.name.as_str(), r)).collect();
    let mut reached: HashSet<String> = HashSet::new();
    let mut todo: Vec<String> = LEXICAL_RULES
        .iter()
        .chain(STARTS)
        .chain(TOKENS)
        .map(|s| s.to_string())
        .collect();
    while let Some(name) = todo.pop() {
        if reached.insert(name.clone())
            && let Some(rule) = rules.get(name.as_str())
        {
            let mut refs = Vec::new();
            rule.expr.refs(&mut refs);
            todo.extend(refs.into_iter().map(|(name, _)| name));
        }
    }
    for rule in &grammar.rules {
        if !reached.contains(&rule.name) {
            report.warn(&format!(
                "line {}: `{}` is never used",
                rule.line, rule.name
            ));
        }
    }
    // Lexical rules can't refer to themselves: they're regular.
    for rule in grammar.rules.iter().filter(|r| is_lexical(&r.name)) {
        if refers_to(grammar, &rule.name, &rule.name) {
            report.fail(&format!(
                "line {}: `{}` refers to itself: lexical rules can't be recursive",
                rule.line, rule.name
            ));
        }
    }
    if report.failures == failures {
        let lexical = grammar.rules.iter().filter(|r| is_lexical(&r.name)).count();
        report.ok(&format!(
            "{} rules ({lexical} lexical, {} syntactic) fit together",
            grammar.rules.len(),
            grammar.rules.len() - lexical
        ));
    }
}

/// Does rule `from` use rule `target`, directly or not?
fn refers_to(grammar: &Grammar, from: &str, target: &str) -> bool {
    let mut seen = HashSet::new();
    let mut todo = vec![from.to_string()];
    while let Some(name) = todo.pop() {
        let Some(rule) = grammar.rules.iter().find(|r| r.name == name) else {
            continue;
        };
        let mut refs = Vec::new();
        rule.expr.refs(&mut refs);
        for (next, _) in refs {
            if next == target {
                return true;
            }
            if seen.insert(next.clone()) {
                todo.push(next);
            }
        }
    }
    false
}

fn check_lexical(grammar: &Grammar, report: &mut Report) {
    report.title("Lexical grammar (section 2)");
    for samples in samples::LEXICAL {
        if !grammar.rules.iter().any(|r| r.name == samples.rule) {
            report.fail(&format!(
                "{} is not defined yet (section {})",
                samples.rule, samples.section
            ));
            continue;
        }
        let mut failed = 0;
        for &text in samples.accept {
            if !lexical::matches(grammar, samples.rule, text) {
                failed += 1;
                report.fail(&format!("{} must match {text:?}", samples.rule));
            }
        }
        for &text in samples.reject {
            if lexical::matches(grammar, samples.rule, text) {
                failed += 1;
                report.fail(&format!("{} must not match {text:?}", samples.rule));
            }
        }
        if failed == 0 {
            report.ok(&format!(
                "{}: {} samples",
                samples.rule,
                samples.accept.len() + samples.reject.len()
            ));
        }
    }
}

/// The rules used from `start`, if they're all defined; or the first
/// that isn't.
fn bnf_for(grammar: &Grammar, start: &str) -> Result<Bnf, String> {
    if !grammar.rules.iter().any(|r| r.name == start) {
        return Err(format!("`{start}` is not defined yet"));
    }
    let mut seen = HashSet::new();
    let mut todo = vec![start.to_string()];
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) || TOKENS.contains(&name.as_str()) {
            continue;
        }
        let Some(rule) = grammar.rules.iter().find(|r| r.name == name) else {
            return Err(format!("`{name}` is not defined yet"));
        };
        let mut refs = Vec::new();
        rule.expr.refs(&mut refs);
        todo.extend(refs.into_iter().map(|(name, _)| name));
    }
    Ok(Bnf::new(grammar, &[start], TOKENS))
}

/// What the grammar makes of `text`, from `start`.
enum Outcome {
    /// Rejected, and why.
    Rejected(&'static str),
    Accepted,
}

fn run(bnf: &Bnf, start: usize, text: &str) -> Outcome {
    let tokens = match tokens::lex(text) {
        Ok(tokens) => tokens,
        Err(NoTokens::LexicalError) => return Outcome::Rejected("its lexer rejects it"),
        Err(NoTokens::Html) => return Outcome::Rejected("HTML is not part of the spec"),
    };
    let ids: Vec<Option<usize>> = tokens.names.iter().map(|name| bnf.term(name)).collect();
    if recognize(bnf, start, &ids).accepts(start) {
        Outcome::Accepted
    } else {
        Outcome::Rejected("the grammar rejects it")
    }
}

fn check_syntax(grammar: &Grammar, samples: &SyntaxSamples, report: &mut Report) {
    report.title(&format!("{} (section {})", samples.start, samples.section));
    let bnf = match bnf_for(grammar, samples.start) {
        Ok(bnf) => bnf,
        Err(why) => {
            report.fail(&format!("can't check yet: {why}"));
            return;
        }
    };
    let start = bnf.nt(samples.start).expect("the start rule is in the BNF");
    let failures = report.failures;
    let show = |text: &str| text.replace('\n', "⏎");
    for &text in samples.accept {
        match run(&bnf, start, text) {
            Outcome::Accepted => {
                if let Some(ambiguity) = ambiguity(&bnf, start, text) {
                    report.fail(&format!(
                        "`{}` can be read in more than one way",
                        show(text)
                    ));
                    report.note(&format!("one of them: {ambiguity}"));
                }
            }
            Outcome::Rejected(_) => {
                report.fail(&format!("rejects `{}`, which is valid", show(text)));
            }
        }
    }
    for &text in samples.reject {
        if let Outcome::Accepted = run(&bnf, start, text) {
            report.fail(&format!(
                "accepts `{}`, which is not valid Maxim",
                show(text)
            ));
        }
    }
    for &(text, expected) in samples.grouped {
        check_grouping(&bnf, start, text, expected, report);
    }
    if report.failures == failures {
        report.ok(&format!(
            "{} valid and {} invalid samples",
            samples.accept.len() + samples.grouped.len(),
            samples.reject.len()
        ));
    }
}

/// If `text` has two parse trees, one of them, grouped.
fn ambiguity(bnf: &Bnf, start: usize, text: &str) -> Option<String> {
    let tokens = tokens::lex(text).ok()?;
    let ids: Vec<Option<usize>> = tokens.names.iter().map(|name| bnf.term(name)).collect();
    let chart = recognize(bnf, start, &ids);
    let (tree, ambiguous) = chart.tree(start)?;
    ambiguous.then(|| grouping(bnf, &tree, &tokens.texts).display)
}

fn check_grouping(bnf: &Bnf, start: usize, text: &str, expected: &str, report: &mut Report) {
    let Ok(tokens) = tokens::lex(text) else {
        report.fail(&format!("`{text}` doesn't lex (a bug in the samples)"));
        return;
    };
    let ids: Vec<Option<usize>> = tokens.names.iter().map(|name| bnf.term(name)).collect();
    let chart = recognize(bnf, start, &ids);
    let Some((tree, ambiguous)) = chart.tree(start) else {
        report.fail(&format!("rejects `{text}`, which is valid"));
        return;
    };
    let actual = grouping(bnf, &tree, &tokens.texts);
    if ambiguous {
        report.fail(&format!(
            "`{text}` can be read in more than one way: the grammar must say which"
        ));
        report.note(&format!("one of them: {}", actual.display));
        return;
    }
    let wanted = expected_spans(expected);
    let crossing = |(a, b): (usize, usize), (c, d): (usize, usize)| {
        a < c && c < b && b < d || c < a && a < d && d < b
    };
    let missing = wanted.iter().any(|span| !actual.spans.contains(span));
    let crossed = wanted
        .iter()
        .any(|&w| actual.spans.iter().any(|&a| crossing(w, a)));
    if missing || crossed {
        report.fail(&format!("`{text}` groups as {}", actual.display));
        report.note(&format!("expected {expected}"));
    }
}

/// The token ranges of the `⟨ ⟩` groups in `expected`.
fn expected_spans(expected: &str) -> BTreeSet<(usize, usize)> {
    let mut spans = BTreeSet::new();
    let mut open = Vec::new();
    let mut count = 0;
    let mut piece = String::new();
    let flush = |piece: &mut String, count: &mut usize| {
        if !piece.trim().is_empty() {
            let tokens = tokens::lex(piece).ok().expect("sample groups lex");
            *count += tokens.names.len();
        }
        piece.clear();
    };
    for c in expected.chars() {
        match c {
            '⟨' => {
                flush(&mut piece, &mut count);
                open.push(count);
            }
            '⟩' => {
                flush(&mut piece, &mut count);
                let start = open.pop().expect("balanced groups");
                spans.insert((start, count));
            }
            c => piece.push(c),
        }
    }
    spans
}

fn check_programs(grammar: &Grammar, report: &mut Report) {
    report.title("Module: whole programs (section 3.5)");
    let bnf = match bnf_for(grammar, "Module") {
        Ok(bnf) => bnf,
        Err(why) => {
            report.fail(&format!("can't check yet: {why}"));
            return;
        }
    };
    let start = bnf.nt("Module").expect("Module is in the BNF");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    for (dir, valid) in [("ok", true), ("err", false)] {
        let failures = report.failures;
        let mut count = 0;
        for path in programs(&corpus.join(dir)) {
            let text = std::fs::read_to_string(&path).expect("read a corpus program");
            let name = format!(
                "corpus/{dir}/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            count += 1;
            match (run(&bnf, start, &text), valid) {
                (Outcome::Accepted, false) => {
                    report.fail(&format!("accepts {name}, which is not valid Maxim"));
                }
                (Outcome::Rejected(why), true) => {
                    report.fail(&format!("rejects {name}, which is valid: {why}"));
                }
                _ => {}
            }
        }
        if report.failures == failures {
            let what = if valid {
                "valid programs accepted"
            } else {
                "invalid programs rejected"
            };
            report.ok(&format!("{count} {what}"));
        }
    }
}

fn programs(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "mx"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn report_ll1(grammar: &Grammar, report: &mut Report) {
    report.title("Lookahead: where one token isn't enough to choose (LL(1))");
    let bnf = match bnf_for(grammar, "Module") {
        Ok(bnf) => bnf,
        Err(why) => {
            report.warn(&format!("can't analyze yet: {why}"));
            return;
        }
    };
    let start = bnf.nt("Module").expect("Module is in the BNF");
    for rule in ll1::left_recursive(&bnf) {
        report.warn(&format!(
            "`{rule}` is left-recursive: a recursive descent parser would loop on it"
        ));
    }
    let conflicts = ll1::conflicts(&bnf, start);
    if conflicts.is_empty() {
        report.ok("one token of lookahead always decides: the grammar is LL(1)");
    }
    for conflict in conflicts {
        report.warn(&format!("`{}`: {}", conflict.rule, conflict.what));
    }
}
