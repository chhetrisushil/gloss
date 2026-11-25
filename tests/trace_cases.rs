//! Every `tests/trace_cases/<name>.gloss` is built with `build_trace_executable` and run as a
//! real executable. Beside it: `.trace` (the exact standard output) and `.out` (the narration,
//! compared from the narrator on) are required; `.in` (standard input), `.status` (default 0),
//! `.err` (standard error, default empty) and `.files/` (copied into the working folder first)
//! are optional.

mod run_support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use run_support::{DEADLINE, TempDir, copy_tree, run_case};

include!("trace_sweep.inc");

fn cases() -> Vec<PathBuf> {
    trace_sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_cases")).unwrap()
}

#[test]
fn the_sweep_finds_the_trace_cases() {
    assert!(cases().len() >= MIN_TRACE_CASES, "too few trace cases");
}

#[test]
fn the_trace_sweep_rejects_unknown_suffixes() {
    let dir = TempDir::new("trace-sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.trace"), "").unwrap();
    std::fs::write(dir.0.join("a.out"), "").unwrap();
    assert_eq!(trace_sweep(&dir.0).unwrap().len(), 1);
    for ok in ["a.in", "a.err", "a.status"] {
        std::fs::write(dir.0.join(ok), "").unwrap();
    }
    std::fs::create_dir(dir.0.join("a.files")).unwrap();
    assert_eq!(trace_sweep(&dir.0).unwrap().len(), 1);
    for orphan in ["x.trace", "x.out", "b.GLOSS", "a.txt", "a.allow", "a.after"] {
        std::fs::write(dir.0.join(orphan), "").unwrap();
        let err = trace_sweep(&dir.0).unwrap_err();
        assert!(err.contains(orphan), "{err}");
        std::fs::remove_file(dir.0.join(orphan)).unwrap();
    }
}

#[test]
fn a_case_without_a_trace_or_an_out_is_reported() {
    let dir = TempDir::new("trace-sweep-missing");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    let err = trace_sweep(&dir.0).unwrap_err();
    assert!(
        err.contains("no .trace") && err.contains("no .out"),
        "{err}"
    );
}

#[test]
fn a_folder_named_like_a_case_file_is_reported() {
    let dir = TempDir::new("trace-sweep-folder");
    std::fs::write(dir.0.join("x.gloss"), "").unwrap();
    std::fs::write(dir.0.join("x.trace"), "").unwrap();
    std::fs::write(dir.0.join("x.out"), "").unwrap();
    assert_eq!(trace_sweep(&dir.0).unwrap().len(), 1);
    std::fs::create_dir(dir.0.join("x.in")).unwrap();
    assert!(trace_sweep(&dir.0).unwrap_err().contains("x.in"));
}

fn beside(case: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(case.with_extension(ext)).ok()
}

#[test]
fn every_trace_case_matches_on_linux() {
    assert!(cases().len() >= MIN_TRACE_CASES, "too few trace cases");
    let dir = TempDir::new("trace-cases");
    for case in cases() {
        let name = case.file_stem().unwrap().to_str().unwrap();
        let text = std::fs::read_to_string(&case).unwrap();
        let bytes = gloss::build_trace_executable(&text)
            .unwrap_or_else(|d| panic!("{name}: does not build: {d:?}"));
        let exe = dir.0.join(name);
        std::fs::write(&exe, bytes).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let work = dir.0.join(format!("{name}-work"));
        std::fs::create_dir_all(&work).unwrap();
        if case.with_extension("files").is_dir() {
            copy_tree(&case.with_extension("files"), &work);
        }
        let input = beside(&case, "in").unwrap_or_default();
        let Some(ran) = run_case(&exe, &input, DEADLINE, &work, &[], None) else {
            return;
        };
        let status = beside(&case, "status")
            .map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
        let want = (
            status,
            beside(&case, "trace").expect("every case has a .trace file"),
            beside(&case, "err").unwrap_or_default(),
        );
        assert_eq!(ran.unwrap(), want, "{name}");
    }
}

#[test]
fn a_call_nested_too_deep_ends_on_the_enter_then_the_failure_on_linux() {
    let dir = TempDir::new("trace-deep");
    let exe = dir.0.join("deep");
    std::fs::write(&exe, gloss::build_trace_executable(DEEP_PROGRAM).unwrap()).unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let Some(ran) = run_case(&exe, b"", DEADLINE, &dir.0, &[], None) else {
        return;
    };
    let (status, stdout, _) = ran.unwrap();
    assert_eq!((status, deep_ending(&stdout)), (2, DEEP_ENDING.to_string()));
}
