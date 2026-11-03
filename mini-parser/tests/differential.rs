//! Differential tests: your parser against the reference Maxim parser
//! (the one in `../example`, generated from a yacc grammar). For every
//! program, both must agree on one thing: is it valid Maxim? Error
//! messages and trees may differ; the verdict may not.
//!
//! Programs with HTML elements are skipped: HTML is outside this step.

use std::path::Path;

use maxim_compiler::lexer::lex_tokens;
use maxim_compiler::lexer::token::TokenKind;
use maxim_compiler::parser::parse_module;
use maxim_compiler::span::FileId;
use proptest::prelude::*;

/// Is `text` valid Maxim, according to the reference parser? `None` if
/// it has HTML in it.
fn reference_verdict(text: &str) -> Option<bool> {
    if let Ok(tokens) = lex_tokens(FileId(0), text) {
        let html = tokens.iter().any(|token| {
            matches!(
                token.kind,
                TokenKind::JsxTagStart(_)
                    | TokenKind::JsxClose(_)
                    | TokenKind::JsxTagEnd
                    | TokenKind::JsxSelfClose
                    | TokenKind::JsxAttrName(_)
                    | TokenKind::JsxText(_)
                    | TokenKind::DollarBrace
            )
        });
        if html {
            return None;
        }
    }
    Some(parse_module(FileId(0), text).is_ok())
}

/// Is `text` valid Maxim, according to your parser?
fn our_verdict(text: &str) -> bool {
    mini_parser::parse(text).errors().is_empty()
}

fn check_agreement(text: &str) {
    if let Some(reference) = reference_verdict(text) {
        let ours = our_verdict(text);
        assert_eq!(
            ours,
            reference,
            "the reference parser says this program is {}, yours says {}:\n{text}\nyour errors: {:#?}",
            if reference { "valid" } else { "invalid" },
            if ours { "valid" } else { "invalid" },
            mini_parser::parse(text).errors(),
        );
    }
}

fn corpus(dir: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(dir);
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mx"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&path).unwrap())
        })
        .collect();
    files.sort();
    files
}

#[test]
fn the_valid_corpus_is_valid_for_both_parsers() {
    for (name, text) in corpus("ok") {
        assert_eq!(
            reference_verdict(&text),
            Some(true),
            "{name}: the reference rejects it"
        );
        assert!(
            our_verdict(&text),
            "{name}: your parser rejects it:\n{:#?}",
            mini_parser::parse(&text).errors()
        );
    }
}

#[test]
fn the_invalid_corpus_is_invalid_for_both_parsers() {
    for (name, text) in corpus("err") {
        assert_ne!(
            reference_verdict(&text),
            Some(true),
            "{name}: the reference accepts it"
        );
        assert!(!our_verdict(&text), "{name}: your parser accepts it");
    }
}

/// Tokens to insert or swap in: a bit of everything Maxim has.
const VOCABULARY: &[&str] = &[
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
    "x",
    "y",
    "Foo",
    "Bar",
    "_",
    "0",
    "1.5",
    "\"s\"",
    "\"a${x}b\"",
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
    "\n",
    "\n(",
    " (",
    "<x",
    "\n<x",
    "\"a${",
    "}b\"",
    "}c${",
    "/* c */",
    "/* \n */",
    "// c\n",
    "@",
    "&",
];

/// One edit of a program, at a position given as a fraction of its
/// tokens.
#[derive(Clone, Debug)]
enum Edit {
    Delete(f64),
    Duplicate(f64),
    Insert(f64, &'static str),
    Replace(f64, &'static str),
}

fn edit() -> impl Strategy<Value = Edit> {
    let token = proptest::sample::select(VOCABULARY);
    prop_oneof![
        (0.0..1.0).prop_map(Edit::Delete),
        (0.0..1.0).prop_map(Edit::Duplicate),
        (0.0..1.0, token.clone()).prop_map(|(at, t)| Edit::Insert(at, t)),
        (0.0..1.0, token).prop_map(|(at, t)| Edit::Replace(at, t)),
    ]
}

/// Apply edits to a program's tokens (trivia stays around them), and
/// return the new text.
fn mutate(text: &str, edits: &[Edit]) -> String {
    let lexed = mini_parser::lexer::lex(text);
    let mut tokens: Vec<String> = (0..lexed.len())
        .map(|i| lexed.text(i).to_string())
        .collect();
    for edit in edits {
        let real: Vec<usize> = (0..tokens.len())
            .filter(|&i| !tokens[i].trim().is_empty() && !tokens[i].starts_with("//"))
            .collect();
        if real.is_empty() {
            break;
        }
        let pick = |at: f64| real[((at * real.len() as f64) as usize).min(real.len() - 1)];
        match *edit {
            Edit::Delete(at) => {
                tokens.remove(pick(at));
            }
            Edit::Duplicate(at) => {
                let i = pick(at);
                tokens.insert(i, format!("{} ", tokens[i]));
            }
            Edit::Insert(at, token) => tokens.insert(pick(at), format!("{token} ")),
            Edit::Replace(at, token) => {
                let i = pick(at);
                tokens[i] = token.to_string();
            }
        }
    }
    tokens.concat()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 5000, ..ProptestConfig::default() })]

    /// Small edits to valid programs: right at the border between valid
    /// and invalid, where parsers disagree when one of them is wrong.
    #[test]
    fn both_parsers_agree_on_edited_programs(
        file in 0usize..100,
        edits in proptest::collection::vec(edit(), 1..4),
    ) {
        let programs = corpus("ok");
        let (_, text) = &programs[file % programs.len()];
        check_agreement(&mutate(text, &edits));
    }

    /// Random token soup after a module header: mostly invalid, and full
    /// of combinations nobody would write.
    #[test]
    fn both_parsers_agree_on_token_soup(
        tokens in proptest::collection::vec(proptest::sample::select(VOCABULARY), 0..30),
    ) {
        check_agreement(&format!("module M\n\nlet v = {}", tokens.join(" ")));
        check_agreement(&format!("module M\n\nfn f() {{\n  {}\n}}", tokens.join(" ")));
        check_agreement(&format!("module M\n\n{}", tokens.join(" ")));
    }
}
