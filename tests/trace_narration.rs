//! Every `tests/trace_cases/<name>.trace` is fed line by line to a `Narrator` built from
//! `<name>.gloss`: standard output must equal `<name>.out`, and standard error the failure line
//! of a `fail` event (nothing when there is none). Needs no runner, so it runs on every host.

#![allow(dead_code)] // trace_sweep.inc is shared with harnesses that use the deep case

use std::path::{Path, PathBuf};

use gloss::trace::event::{Event, decode};
use gloss::trace::narrate::Narrator;

include!("trace_sweep.inc");

fn cases() -> Vec<PathBuf> {
    trace_sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_cases")).unwrap()
}

/// The narration of one case: (stdout, stderr).
fn narrate(case: &Path) -> (String, String) {
    let text = std::fs::read_to_string(case).unwrap();
    let (diags, checked) = gloss::check_program(&text);
    let checked = checked.unwrap_or_else(|| panic!("{}: {diags:?}", case.display()));
    let mut narrator = Narrator::new(checked.program());
    let trace = std::fs::read_to_string(case.with_extension("trace")).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    for line in trace.lines() {
        narrator
            .line(line, &mut out, &mut err)
            .unwrap()
            .unwrap_or_else(|e| panic!("{}: {e} in {line}", case.display()));
    }
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// What standard error must hold: the failure line of the trace's `fail` event, if any.
fn expected_err(case: &Path) -> String {
    let trace = std::fs::read_to_string(case.with_extension("trace")).unwrap();
    match trace.lines().last().map(decode) {
        Some(Ok(Event::Fail {
            line,
            failure,
            message,
        })) => format!("line {line}: failure {failure}: {message}\n"),
        _ => String::new(),
    }
}

#[test]
fn the_sweep_finds_the_trace_cases() {
    assert!(cases().len() >= MIN_TRACE_CASES, "too few trace cases");
}

#[test]
fn every_trace_is_narrated_as_its_out_says() {
    let mut compared = 0;
    let mut wrong = Vec::new();
    for case in cases() {
        let (out, err) = narrate(&case);
        let want = std::fs::read_to_string(case.with_extension("out")).unwrap();
        if out != want || err != expected_err(&case) {
            wrong.push(format!(
                "{}:\n--- narrated\n{out}{err}--- expected\n{want}{}",
                case.display(),
                expected_err(&case)
            ));
        }
        compared += 1;
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(compared, cases().len());
    assert!(compared >= MIN_TRACE_CASES);
}
