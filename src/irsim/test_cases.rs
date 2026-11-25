//! The `tests/test_cases` cases, run through `lower_tests` and `irsim`: the real executable
//! must agree.

use std::path::{Path, PathBuf};

use crate::check::check;
use crate::irsim::{Faults, Files, run_with};
use crate::lower::lower_tests;
use crate::source::Source;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test_sweep.inc"));

/// Runs one case under irsim and compares its status, standard output and standard error with
/// the expected ones; `out` is the expected standard output, so a test can pass a wrong one.
fn compare(case: &Path, out: &[u8]) -> Result<(), String> {
    let beside = |ext: &str| std::fs::read(case.with_extension(ext)).ok();
    let text = std::fs::read_to_string(case).unwrap();
    let src = Source::new(&text);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    let module = lower_tests(&program, &types, &src).expect("lowers");
    let o = run_with(
        &module,
        "_start",
        b"",
        &[],
        Faults::default(),
        &mut Files::default(),
    );
    let status =
        beside("status").map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
    let want = (status, out.to_vec(), beside("err").unwrap_or_default());
    let got = (o.status, o.stdout, o.stderr);
    if got == want {
        Ok(())
    } else {
        Err(format!("{case:?}: got {got:?}, want {want:?}"))
    }
}

fn cases() -> Vec<PathBuf> {
    test_sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_cases")).unwrap()
}

#[test]
fn every_test_case_gives_the_expected_output_in_irsim() {
    let cases = cases();
    assert!(cases.len() >= MIN_TEST_CASES, "too few test cases");
    for case in cases {
        let out = std::fs::read(case.with_extension("out")).expect("every case has an .out file");
        if let Err(problem) = compare(&case, &out) {
            panic!("{problem}");
        }
    }
}

#[test]
fn the_irsim_harness_is_not_vacuous() {
    let case = cases().into_iter().next().expect("a case");
    assert!(compare(&case, b"not what the case prints\n").is_err());
}
