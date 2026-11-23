//! The `tests/run` cases, run through `lower` and `irsim`: the real executable must agree.

use std::path::Path;

use crate::check::check;
use crate::irsim::run;
use crate::lower::lower;
use crate::source::Source;

#[test]
fn every_run_case_gives_the_expected_output_in_irsim() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run");
    for entry in std::fs::read_dir(dir).unwrap() {
        let case = entry.unwrap().path();
        if case.extension().is_none_or(|x| x != "gloss") {
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
}
