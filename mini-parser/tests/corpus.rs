//! Snapshot tests, the way rust-analyzer tests its parser: next to each
//! program in `corpus/`, a file holds what the parser must make of it.
//!
//! - `corpus/ok/NAME.mx` is valid Maxim: `NAME.tree` is its syntax tree.
//! - `corpus/err/NAME.mx` is not: `NAME.error` is the first error.
//!
//! When a test fails, it shows the difference between the expected and
//! the actual output. Don't run these with `UPDATE_EXPECT=1`: that
//! overwrites the expected files with whatever your parser produces.

use std::path::{Path, PathBuf};

use expect_test::expect_file;

fn programs(dir: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(dir);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mx"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn valid_programs_give_the_expected_trees() {
    for path in programs("ok") {
        let text = std::fs::read_to_string(&path).unwrap();
        let parse = mini_parser::parse(&text);
        assert!(
            parse.errors().is_empty(),
            "{}: unexpected errors:\n{:#?}",
            path.display(),
            parse.errors()
        );
        expect_file![path.with_extension("tree")].assert_eq(&parse.debug_dump());
    }
}

#[test]
fn invalid_programs_give_the_expected_first_error() {
    for path in programs("err") {
        let text = std::fs::read_to_string(&path).unwrap();
        let parse = mini_parser::parse(&text);
        let first = parse
            .errors()
            .first()
            .unwrap_or_else(|| panic!("{}: no error, but the program is invalid", path.display()));
        expect_file![path.with_extension("error")].assert_eq(&format!("{first}\n"));
    }
}
