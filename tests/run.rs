//! Every `tests/run/<name>.gloss` is built by the real `gloss` binary and run on Linux.
//! Beside it: `.out` (required), and optional `.in`, `.err` and `.status`.

mod run_support;

use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{TempDir, run};

include!("run_sweep.inc");

fn cases() -> Vec<PathBuf> {
    sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run")).unwrap()
}

fn beside(case: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(case.with_extension(ext)).ok()
}

#[test]
fn the_sweep_finds_the_run_cases() {
    assert!(cases().len() >= MIN_CASES, "too few run cases");
}

#[test]
fn the_sweep_reports_an_orphan() {
    let dir = TempDir::new("sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.out"), "").unwrap();
    assert_eq!(sweep(&dir.0).unwrap().len(), 1);
    for orphan in ["x.out", "b.GLOSS", "a.txt"] {
        std::fs::write(dir.0.join(orphan), "").unwrap();
        let err = sweep(&dir.0).unwrap_err();
        assert!(err.contains(orphan), "{err}");
        std::fs::remove_file(dir.0.join(orphan)).unwrap();
    }
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
