//! Golden tests, mirroring `compiler/test/GoldenTests.hs` in the sibling
//! `language-project` checkout: the fixture INPUTS are shared with the
//! Haskell compiler
//! (`compiler/test/fixtures/golden/{diagnostics,codegen}/<case>/*.mx`),
//! but the expected files are Rust-owned (`tests/golden/…`)
//! — the conformance bar is behavioral, so diagnostic prose and JS
//! formatting may drift from the Haskell snapshots, and each Rust
//! snapshot was hand-reviewed against its Haskell counterpart when first
//! generated (codes, spans, and hints must agree).
//!
//! Regenerate with `UPDATE_GOLDENS=1 cargo test --test golden` and
//! review the diff — these files are curated UX artifacts.

use std::path::{Path, PathBuf};

use maxim_compiler::diag::render::{mk_source_map, render_diagnostics};
use maxim_compiler::driver::{CompileOptions, DiskLoader, build_entry};

/// The Haskell repo, which owns the fixtures and the stdlib — expected
/// beside this workspace, two levels up. Relative to the package root
/// (cargo runs integration tests there), so snapshot headers carry
/// stable machine-independent paths.
fn repo_root() -> PathBuf {
    PathBuf::from("../../language-project")
}

fn fixture_root() -> PathBuf {
    repo_root().join("compiler/test/fixtures/golden")
}

fn expected_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn stdlib_dir() -> PathBuf {
    repo_root().join("stdlib/core")
}

fn cases_under(group: &str) -> Vec<String> {
    let mut cases: Vec<String> = std::fs::read_dir(fixture_root().join(group))
        .unwrap_or_else(|e| panic!("fixture group {group}: {e}"))
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    cases.sort();
    cases
}

fn scratch_dir(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("maxim-rust-golden").join(case);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[track_caller]
fn check_golden(name: &str, expected_path: &Path, actual: &str) {
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(expected_path.parent().unwrap()).unwrap();
        std::fs::write(expected_path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(expected_path).unwrap_or_else(|_| {
        panic!(
            "{name}: missing golden {} — run UPDATE_GOLDENS=1 cargo test --test golden",
            expected_path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "{name}: output differs from {}",
        expected_path.display()
    );
}

#[test]
fn diagnostics_goldens() {
    for case in cases_under("diagnostics") {
        let case_dir = fixture_root().join("diagnostics").join(&case);
        let mut loader = DiskLoader;
        let outcome = build_entry(
            &mut loader,
            &CompileOptions {
                entry: case_dir.join("Main.mx"),
                out_dir: scratch_dir(&case),
                stdlib_dir: stdlib_dir(),
                check_only: true,
            },
        );
        let srcs = mk_source_map(&outcome.sources);
        let rendered = render_diagnostics(false, &srcs, &outcome.diags);
        check_golden(
            &case,
            &expected_root()
                .join("diagnostics")
                .join(&case)
                .join("expected.txt"),
            &rendered,
        );
    }
}

#[test]
fn codegen_goldens() {
    for case in cases_under("codegen") {
        let case_dir = fixture_root().join("codegen").join(&case);
        let out_dir = scratch_dir(&case);
        let mut loader = DiskLoader;
        let outcome = build_entry(
            &mut loader,
            &CompileOptions {
                entry: case_dir.join("Main.mx"),
                out_dir: out_dir.clone(),
                stdlib_dir: stdlib_dir(),
                check_only: false,
            },
        );
        if !outcome.succeeded {
            let srcs = mk_source_map(&outcome.sources);
            panic!(
                "{case}: expected a clean build, got:\n{}",
                render_diagnostics(false, &srcs, &outcome.diags)
            );
        }
        // Snapshot only the case's own modules (basenames of its .mx
        // files) — the stdlib, runtime, and _main.mjs are excluded.
        let mut own: Vec<String> = std::fs::read_dir(&case_dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension().is_some_and(|x| x == "mx"))
                    .then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
            })
            .collect();
        own.sort();
        let mut parts: Vec<String> = Vec::new();
        for name in own {
            let file = format!("{name}.mjs");
            let contents = std::fs::read_to_string(out_dir.join(&file))
                .unwrap_or_else(|e| panic!("{case}: missing {file}: {e}"));
            parts.push(format!("// ==== {file}\n{contents}"));
        }
        let combined = parts.join("\n");
        check_golden(
            &case,
            &expected_root()
                .join("codegen")
                .join(&case)
                .join("expected.mjs"),
            &combined,
        );
    }
}
