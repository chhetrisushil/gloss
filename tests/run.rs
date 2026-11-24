//! Every `tests/run/<name>.gloss` is built by the real `gloss` binary and run on Linux.
//! Beside it: `.out` (required), and optional `.in`, `.err`, `.status` and `.ulimit` (a
//! virtual-memory limit in KiB, applied with `ulimit -v` in the shell that runs the program).

mod run_support;

use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{DEADLINE, TempDir, run_case, run_env};

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
    for orphan in ["x.out", "b.GLOSS", "a.txt", "x.ulimit"] {
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
        let kib = beside(&case, "ulimit").map(|k| {
            let text = String::from_utf8(k).unwrap();
            text.trim()
                .parse::<u64>()
                .unwrap_or_else(|e| panic!("{name}.ulimit: {e}"))
        });
        let input = beside(&case, "in").unwrap_or_default();
        // The program runs in its own folder, `work/`, beside the executable.
        let work = dir.0.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let Some(ran) = run_case(&dir.0.join(name), &input, DEADLINE, &work, &[], kib) else {
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

/// A large environment sits above the stack pointer at entry; the limit must still be measured
/// from the top of the stack, so deep recursion fails with code 10 instead of faulting.
#[test]
fn too_deep_still_fails_cleanly_with_a_huge_environment() {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/too_deep.gloss");
    let dir = TempDir::new("run-big-env");
    let source = dir.0.join("too_deep.gloss");
    std::fs::copy(&case, &source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    // Four variables of 60000 bytes: a single string is limited to 128 KiB.
    let vars: Vec<(&str, String)> = ["BIG1", "BIG2", "BIG3", "BIG4"]
        .map(|k| (k, "x".repeat(60000)))
        .to_vec();
    let Some(ran) = run_env(&dir.0.join("too_deep"), b"", DEADLINE, &vars) else {
        return;
    };
    let want = (2, Vec::new(), beside(&case, "err").unwrap());
    assert_eq!(ran.unwrap(), want);
}
