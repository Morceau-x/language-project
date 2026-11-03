//! Properties that hold for any text at all, valid Maxim or not:
//!
//! - lexing and parsing never panic;
//! - the tokens, and the syntax tree, contain every byte of the text,
//!   once, in order;
//! - every error points inside the text.
//!
//! This is what lets an editor use the parser on code being typed.

use proptest::prelude::*;

fn check(text: &str) {
    let lexed = mini_parser::lexer::lex(text);
    let tokens: String = (0..lexed.len()).map(|i| lexed.text(i)).collect();
    assert_eq!(tokens, text, "the tokens lost or added text");

    let parse = mini_parser::parse(text);
    assert_eq!(
        parse.syntax_node().text(),
        text,
        "the tree lost or added text"
    );
    for error in parse.errors() {
        assert!(
            error.range().end() as usize <= text.len(),
            "{error} points past the end"
        );
    }
}

/// Text that looks a bit like Maxim: its characters, and a few others.
fn maxim_ish() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r#"(module|fn|let|type|match|if|else|[a-zA-Z_0-9]|[ \n\t]|\r\n|[(){}\[\]<>,.:;=|&!?+*/%~^@#$"\\-]|\$\{|é|日|\r){0,60}"#,
    )
    .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 3000, ..ProptestConfig::default() })]

    #[test]
    fn any_maxim_ish_text(text in maxim_ish()) {
        check(&text);
        check(&format!("module M\n\nfn f() {{\n{text}\n}}\n"));
    }

    #[test]
    fn any_text_at_all(text in any::<String>()) {
        check(&text);
    }
}

#[test]
fn edge_cases() {
    for text in [
        "",
        " ",
        "module",
        "module M",
        "\"",
        "\"${",
        "\"${\"${",
        "}",
        "\"a${}",
        "/*",
        "\r",
        "(((((((((((",
        "module M\nlet x = ",
        "module M\nfn f(",
        "module M\nlet x = [1, ..",
        "module M\nlet x = match",
        "module M\nlet x = \"${ { }",
    ] {
        check(text);
    }
}
