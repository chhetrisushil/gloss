//! Every `tests/run/<name>.gloss` is built by the real `gloss` binary and run on Linux.
//! Beside it: `.out` (required), and optional `.in`, `.err` and `.status`.

mod run_support;

use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{TempDir, run};

/// Raised by every task that adds cases.
const MIN_CASES: usize = 2;

fn cases() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "gloss"))
        .collect();
    found.sort();
    found
}

fn beside(case: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(case.with_extension(ext)).ok()
}

#[test]
fn the_sweep_finds_the_run_cases() {
    assert!(cases().len() >= MIN_CASES, "too few run cases");
}

#[test]
fn every_run_case_builds_and_runs() {
    for case in cases() {
        let name = case.file_stem().unwrap().to_str().unwrap();
        let dir = TempDir::new(&format!("run-{name}"));
        let source = dir.0.join(format!("{name}.gloss"));
        std::fs::copy(&case, &source).unwrap();
        let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
            .arg("build")
            .arg(&source)
            .output()
            .unwrap();
        assert_eq!(built.status.code(), Some(0), "{name}: {built:?}");
        let Some(ran) = run(&dir.0.join(name), &beside(&case, "in").unwrap_or_default()) else {
            continue;
        };
        let status = beside(&case, "status")
            .map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
        let want = (
            status,
            beside(&case, "out").expect("every case has an .out file"),
            beside(&case, "err").unwrap_or_default(),
        );
        assert_eq!(ran.unwrap(), want, "{name}");
    }
}
