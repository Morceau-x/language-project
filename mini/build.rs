//! Build script: Cargo runs it before compiling the crate.
//!
//! It feeds the lexer and grammar in `src/yacc/` to grmtools, which
//! generates the Rust code of a lexer and an LR(1) parser into Cargo's
//! `OUT_DIR`. `src/yacc/mod.rs` then includes that generated code with
//! `lrlex_mod!` and `lrpar_mod!`. Any error in the grammar, such as a
//! shift/reduce conflict, stops the build right here.

use lrlex::CTLexerBuilder;

fn main() {
    let built = CTLexerBuilder::new()
        .lrpar_config(|parser| {
            parser
                .grammar_in_src_dir("yacc/mini.y")
                .expect("cannot read src/yacc/mini.y")
        })
        .lexer_in_src_dir("yacc/mini.l")
        .expect("cannot read src/yacc/mini.l")
        // `NEG` exists only to give unary minus its precedence (`%prec
        // NEG`): the lexer never produces it, and that's fine.
        .allow_missing_terms_in_lexer(true)
        .build();
    if let Err(error) = built {
        // Print the grammar error and fail the build.
        panic!("{error}");
    }
}
