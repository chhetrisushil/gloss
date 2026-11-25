//! The `tests/trace_cases` cases, run through `lower_trace` and `irsim`: the real executable
//! must agree.

use std::path::{Path, PathBuf};

use crate::check::check;
use crate::irsim::{Faults, Files, run_with};
use crate::lower::lower_trace;
use crate::source::Source;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/trace_sweep.inc"
));

/// Runs one case under irsim and compares its status, standard output and standard error with
/// the expected ones; `trace` is the expected standard output, so a test can pass a wrong one.
fn compare(case: &Path, trace: &[u8]) -> Result<(), String> {
    let beside = |ext: &str| std::fs::read(case.with_extension(ext)).ok();
    let text = std::fs::read_to_string(case).unwrap();
    let src = Source::new(&text);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    let module = lower_trace(&program, &types, &src).expect("lowers");
    let mut files = Files::default();
    let seeded = case.with_extension("files");
    if seeded.is_dir() {
        super::cases::seed_files(&mut files, &seeded);
    }
    let input = beside("in").unwrap_or_default();
    let o = run_with(
        &module,
        "_start",
        &input,
        &[],
        Faults::default(),
        &mut files,
    );
    let status =
        beside("status").map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
    let want = (status, trace.to_vec(), beside("err").unwrap_or_default());
    let got = (o.status, o.stdout, o.stderr);
    if got == want {
        Ok(())
    } else {
        Err(format!("{case:?}: got {got:?}, want {want:?}"))
    }
}

fn cases() -> Vec<PathBuf> {
    trace_sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_cases")).unwrap()
}

#[test]
fn every_trace_case_matches_in_irsim() {
    let cases = cases();
    assert!(cases.len() >= MIN_TRACE_CASES, "too few trace cases");
    for case in cases {
        let trace =
            std::fs::read(case.with_extension("trace")).expect("every case has a .trace file");
        if let Err(problem) = compare(&case, &trace) {
            panic!("{problem}");
        }
    }
}

#[test]
fn the_irsim_trace_harness_is_not_vacuous() {
    let case = cases().into_iter().next().expect("a case");
    assert!(compare(&case, b"not what the case prints\n").is_err());
}

#[test]
fn a_call_nested_too_deep_ends_on_the_enter_then_the_failure_in_irsim() {
    let src = Source::new(DEEP_PROGRAM);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    let module = lower_trace(&program, &types, &src).expect("lowers");
    let o = run_with(
        &module,
        "_start",
        b"",
        &[],
        Faults::default(),
        &mut Files::default(),
    );
    assert_eq!(
        (o.status, deep_ending(&o.stdout)),
        (2, DEEP_ENDING.to_string())
    );
}
