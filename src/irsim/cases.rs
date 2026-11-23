//! The `tests/run` cases, run through `lower` and `irsim`: the real executable must agree.

use std::path::{Path, PathBuf};

use crate::check::check;
use crate::irsim::run;
use crate::lower::lower;
use crate::source::Source;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run_sweep.inc"));

#[test]
fn every_run_case_gives_the_expected_output_in_irsim() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run");
    let cases = sweep(&dir).unwrap();
    assert!(cases.len() >= MIN_CASES, "too few run cases");
    for case in cases {
        let beside = |ext: &str| std::fs::read(case.with_extension(ext)).ok();
        let text = std::fs::read_to_string(&case).unwrap();
        let src = Source::new(&text);
        let program = crate::parse_source(&src).expect("parses");
        let types = check(&program, &src).types.expect("checks clean");
        let module = lower(&program, &types, &src).expect("lowers");
        let o = run(&module, "_start", &beside("in").unwrap_or_default());
        let status =
            beside("status").map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
        let want = (
            status,
            beside("out").expect("every case has an .out file"),
            beside("err").unwrap_or_default(),
        );
        assert_eq!((o.status, o.stdout, o.stderr), want, "{case:?}");
    }
}

#[test]
fn the_sweep_reports_an_orphan() {
    let dir = std::env::temp_dir().join(format!("gloss-sweep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.out"), "").unwrap();
    let err = sweep(&dir).unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(err.contains("x.out"), "{err}");
}
