//! Every `tests/explain_cases/<name>.gloss` is explained, and the result must equal `<name>.out`
//! beside it: the listing up to and including the `runtime` line, then a blank line and the name
//! of each runtime routine, one per line (D57; the routine bodies are not golden, so a runtime
//! change does not rewrite every case). Run with `GLOSS_BLESS=1` to rewrite the `.out` files; the
//! test then fails, so a bless run never passes silently.

mod run_support;

use std::path::{Path, PathBuf};

use run_support::TempDir;

include!("explain_sweep.inc");

fn blessing() -> bool {
    std::env::var_os("GLOSS_BLESS").is_some_and(|v| v == "1")
}

fn cases() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/explain_cases");
    explain_sweep(&dir, blessing()).unwrap()
}

/// The text `<name>.out` holds for the listing `full` of the program `name.gloss`.
fn golden(full: &str) -> String {
    let mut out = String::new();
    let mut in_runtime = false;
    for line in full.lines() {
        if !in_runtime {
            out.push_str(line);
            out.push('\n');
            if line == "runtime" {
                in_runtime = true;
                out.push('\n');
            }
        } else if let Some(name) = line.strip_prefix("routine ") {
            out.push_str(name);
            out.push('\n');
        }
    }
    out
}

fn explained(case: &Path) -> String {
    let name = case.file_name().unwrap().to_str().unwrap();
    let text = std::fs::read_to_string(case).unwrap();
    let (diagnostics, checked) = gloss::check_program(&text);
    let checked = checked.unwrap_or_else(|| panic!("{name}: does not check: {diagnostics:?}"));
    let full = checked
        .explanation(name)
        .unwrap_or_else(|d| panic!("{name}: cannot be explained: {d:?}"));
    golden(&full)
}

#[test]
fn the_sweep_finds_the_explain_cases() {
    assert!(cases().len() >= MIN_EXPLAIN_CASES, "too few explain cases");
}

#[test]
fn the_explain_sweep_rejects_stray_files() {
    let dir = TempDir::new("explain-sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.out"), "").unwrap();
    assert_eq!(explain_sweep(&dir.0, false).unwrap().len(), 1);
    std::fs::write(dir.0.join(".DS_Store"), "").unwrap();
    assert_eq!(explain_sweep(&dir.0, false).unwrap().len(), 1);
    for stray in ["x.out", "a.trace", "b.GLOSS", "a.txt", "notes"] {
        std::fs::write(dir.0.join(stray), "").unwrap();
        let err = explain_sweep(&dir.0, false).unwrap_err();
        assert!(err.contains(stray), "{err}");
        std::fs::remove_file(dir.0.join(stray)).unwrap();
    }
    std::fs::create_dir(dir.0.join("a.in")).unwrap();
    assert!(explain_sweep(&dir.0, false).unwrap_err().contains("a.in"));
}

#[test]
fn a_case_without_an_out_is_reported() {
    let dir = TempDir::new("explain-sweep-missing");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    let err = explain_sweep(&dir.0, false).unwrap_err();
    assert!(err.contains("no .out"), "{err}");
}

#[test]
fn blessing_accepts_a_program_without_an_out() {
    let dir = TempDir::new("explain-sweep-bless");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    assert_eq!(explain_sweep(&dir.0, true).unwrap().len(), 1);
    std::fs::write(dir.0.join("x.out"), "").unwrap();
    assert!(explain_sweep(&dir.0, true).unwrap_err().contains("x.out"));
}

#[test]
fn the_golden_text_keeps_the_listing_and_names_the_routines() {
    let full = "Explain x.gloss for x86-64 Linux\n\nfeature start (line 1)\n    ret\n\nruntime\n\nroutine a\n    ret\n\nroutine b\n    ret\n";
    assert_eq!(
        golden(full),
        "Explain x.gloss for x86-64 Linux\n\nfeature start (line 1)\n    ret\n\nruntime\n\na\nb\n"
    );
}

/// Rewrites the `.out` of every case in `found`, then returns `Err`, so a bless run never passes.
fn bless_cases(found: &[PathBuf]) -> Result<(), String> {
    for case in found {
        std::fs::write(case.with_extension("out"), explained(case)).unwrap();
    }
    Err(format!(
        "blessed {} files, rerun without GLOSS_BLESS",
        found.len()
    ))
}

#[test]
fn blessing_writes_the_outs_and_still_fails() {
    let dir = TempDir::new("explain-bless-run");
    let case = dir.0.join("a.gloss");
    std::fs::write(&case, "feature start\n  show \"x\"\n").unwrap();
    let err = bless_cases(std::slice::from_ref(&case)).unwrap_err();
    assert!(err.contains("blessed 1 files"), "{err}");
    let out = std::fs::read_to_string(case.with_extension("out")).unwrap();
    assert!(out.contains("feature start"), "{out}");
}

#[test]
fn every_explain_case_matches() {
    let found = cases();
    assert!(found.len() >= MIN_EXPLAIN_CASES, "too few explain cases");
    if blessing() {
        bless_cases(&found).unwrap();
    }
    for case in &found {
        let got = explained(case);
        let out = case.with_extension("out");
        let want = std::fs::read_to_string(&out).unwrap();
        if got != want {
            let at = got
                .lines()
                .zip(want.lines())
                .position(|(g, w)| g != w)
                .unwrap_or_else(|| got.lines().count().min(want.lines().count()));
            panic!(
                "{}: line {} differs\n  got:  {:?}\n  want: {:?}",
                out.display(),
                at + 1,
                got.lines().nth(at),
                want.lines().nth(at)
            );
        }
    }
}
