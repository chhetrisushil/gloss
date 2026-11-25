//! Every `tests/trace_cases/<name>.trace` is fed line by line to a `Narrator` built from
//! `<name>.gloss`: standard output must equal `<name>.out`, and standard error must be written
//! exactly when the case fails. Needs no runner, so it runs on every host.

use std::path::{Path, PathBuf};

use gloss::trace::narrate::Narrator;

/// `trace_sweep.inc` is shared with harnesses that use the deep case; this one does not.
#[allow(dead_code)]
mod sweep {
    use std::path::{Path, PathBuf};

    include!("trace_sweep.inc");

    pub const MIN_CASES: usize = MIN_TRACE_CASES;

    pub fn sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
        trace_sweep(dir)
    }
}
use sweep::{MIN_CASES as MIN_TRACE_CASES, sweep as trace_sweep};

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
        // The failure text itself is checked against a real build's stderr in irsim; here
        // only that the narrator wrote some exactly when the program fails (has a .status).
        let failed = case.with_extension("status").is_file();
        if out != want || err.is_empty() == failed {
            wrong.push(format!(
                "{}:\n--- narrated\n{out}{err}--- expected\n{want}(stderr when failing: {failed})",
                case.display(),
            ));
        }
        compared += 1;
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let on_disk =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_cases"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "gloss")
            })
            .count();
    assert_eq!(compared, on_disk);
    assert!(compared >= MIN_TRACE_CASES);
}
