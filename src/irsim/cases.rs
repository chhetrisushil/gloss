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
    // irsim has no address-space limit, so a case with a `.ulimit` file cannot run here; each
    // skip is counted and must match the files, so a case cannot vanish silently.
    let mut skipped = 0;
    for case in cases {
        if case.with_extension("ulimit").is_file() {
            skipped += 1;
            continue;
        }
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
    let limited = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "ulimit")
        })
        .count();
    assert_eq!(
        skipped, limited,
        "every .ulimit case is skipped, and only those"
    );
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
