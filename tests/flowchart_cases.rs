//! Every `tests/flowchart_cases/<case>.gloss` is charted, and each `(file, svg)` that
//! `Checked::flowcharts` returns must equal the file of that name beside it, `<case>.<feature>.svg`.
//! An expected `.svg` the program does not produce fails, and so does a produced one with no
//! expected file. Run with `GLOSS_BLESS=1` to rewrite and create the `.svg` files; the test then
//! fails, so a bless run never passes silently.

mod run_support;

use std::path::{Path, PathBuf};

use run_support::TempDir;

include!("flowchart_sweep.inc");

fn blessing() -> bool {
    std::env::var_os("GLOSS_BLESS").is_some_and(|v| v == "1")
}

fn case_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/flowchart_cases")
}

/// The charts of the program `case`, or why it has none.
fn charted(case: &Path) -> Result<Vec<(String, String)>, String> {
    let name = case.file_name().unwrap().to_str().unwrap();
    let text = std::fs::read_to_string(case).map_err(|e| format!("{name}: {e}"))?;
    let (diagnostics, checked) = gloss::check_program(&text);
    let first = diagnostics.first().map(|d| d.problem.clone());
    let checked =
        checked.ok_or_else(|| format!("{name} does not check: {}", first.unwrap_or_default()))?;
    Ok(checked.flowcharts(name))
}

/// The first line where `got` and `want` differ, as a message, or `None` when they are equal.
fn first_difference(file: &str, got: &str, want: &str) -> Option<String> {
    if got == want {
        return None;
    }
    if got.lines().eq(want.lines()) {
        let how = if got.ends_with('\n') != want.ends_with('\n') {
            "the final newline differs"
        } else {
            "the files differ only in line endings"
        };
        return Some(format!("{file}: {how}"));
    }
    let at = got
        .lines()
        .zip(want.lines())
        .position(|(g, w)| g != w)
        .unwrap_or_else(|| got.lines().count().min(want.lines().count()));
    let line = |t: &str| t.lines().nth(at).unwrap_or("<end of file>").to_string();
    Some(format!(
        "{file}: line {} differs\n  expected: {:?}\n  got:      {:?}",
        at + 1,
        line(want),
        line(got)
    ))
}

/// The `<case>.*.svg` files in `dir` that `produced` does not include.
fn stale(dir: &Path, case: &Path, produced: &[(String, String)]) -> Vec<String> {
    let prefix = format!("{}.", case.file_stem().unwrap().to_str().unwrap());
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(".svg"))
        .filter(|n| !produced.iter().any(|(file, _)| file == n))
        .collect();
    names.sort();
    names
}

/// What is wrong with the charts `produced` for the case in `dir`, one message per problem.
fn compare(dir: &Path, case: &Path, produced: &[(String, String)]) -> Vec<String> {
    let mut problems = Vec::new();
    for (file, svg) in produced {
        match std::fs::read_to_string(dir.join(file)) {
            Ok(want) => problems.extend(first_difference(file, svg, &want)),
            Err(_) => problems.push(format!("{file}: produced, but there is no expected file")),
        }
    }
    for name in stale(dir, case, produced) {
        problems.push(format!(
            "{name}: expected, but the program does not produce it"
        ));
    }
    problems
}

#[test]
fn the_sweep_finds_the_flowchart_cases() {
    let found = flowchart_sweep(&case_dir(), blessing()).unwrap();
    assert!(
        found.len() >= MIN_FLOWCHART_CASES,
        "too few flowchart cases"
    );
}

#[test]
fn the_flowchart_sweep_rejects_stray_files() {
    let dir = TempDir::new("flowchart-sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.start.svg"), "").unwrap();
    assert_eq!(flowchart_sweep(&dir.0, false).unwrap().len(), 1);
    std::fs::write(dir.0.join(".DS_Store"), "").unwrap();
    assert_eq!(flowchart_sweep(&dir.0, false).unwrap().len(), 1);
    for stray in ["x.start.svg", "a.svg", "a.out", "b.GLOSS", "notes"] {
        std::fs::write(dir.0.join(stray), "").unwrap();
        let err = flowchart_sweep(&dir.0, false).unwrap_err();
        assert!(err.contains(stray), "{err}");
        std::fs::remove_file(dir.0.join(stray)).unwrap();
    }
    std::fs::create_dir(dir.0.join("a.in")).unwrap();
    assert!(flowchart_sweep(&dir.0, false).unwrap_err().contains("a.in"));
}

#[test]
fn a_case_without_an_svg_is_reported() {
    let dir = TempDir::new("flowchart-sweep-missing");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    let err = flowchart_sweep(&dir.0, false).unwrap_err();
    assert!(err.contains("no .svg"), "{err}");
}

#[test]
fn blessing_accepts_a_program_without_an_svg() {
    let dir = TempDir::new("flowchart-sweep-bless");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    assert_eq!(flowchart_sweep(&dir.0, true).unwrap().len(), 1);
    std::fs::write(dir.0.join("x.out"), "").unwrap();
    assert!(flowchart_sweep(&dir.0, true).unwrap_err().contains("x.out"));
}

#[test]
fn the_comparison_names_the_first_differing_line() {
    let msg = first_difference("f.svg", "a\nb\nc\n", "a\nx\nc\n").unwrap();
    assert!(
        msg.contains("line 2") && msg.contains("\"x\"") && msg.contains("\"b\""),
        "{msg}"
    );
    assert!(first_difference("f.svg", "a\n", "a\n").is_none());
}

#[test]
fn a_missing_or_unproduced_svg_is_a_problem() {
    let dir = TempDir::new("flowchart-compare");
    let case = dir.0.join("a.gloss");
    std::fs::write(dir.0.join("a.start.svg"), "x\n").unwrap();
    let produced = vec![("a.start.svg".to_string(), "x\n".to_string())];
    assert!(compare(&dir.0, &case, &produced).is_empty());
    std::fs::write(dir.0.join("a.gone.svg"), "x\n").unwrap();
    let problems = compare(&dir.0, &case, &produced);
    assert!(
        problems.iter().any(|p| p.contains("a.gone.svg")),
        "{problems:?}"
    );
    let more = vec![("a.new.svg".to_string(), "x\n".to_string())];
    let problems = compare(&dir.0, &case, &more);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("a.new.svg") && p.contains("no expected"))
    );
}

/// Checks every case in `found` against `dir`, reporting every failure together. When
/// `bless` is set it rewrites the SVGs, deletes the stale ones, and returns `Err` naming
/// what it did, so a bless run never passes.
fn check_cases(dir: &Path, found: &[PathBuf], bless: bool) -> Result<(), String> {
    let mut problems = Vec::new();
    let mut deleted = Vec::new();
    for case in found {
        let produced = match charted(case) {
            Ok(produced) => produced,
            Err(problem) => {
                problems.push(problem);
                continue;
            }
        };
        if bless {
            for (file, svg) in &produced {
                std::fs::write(dir.join(file), svg).unwrap();
            }
            for name in stale(dir, case, &produced) {
                std::fs::remove_file(dir.join(&name)).unwrap();
                deleted.push(name);
            }
        } else {
            problems.extend(compare(dir, case, &produced));
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    if bless {
        return Err(format!(
            "blessed {} cases, deleted [{}], rerun without GLOSS_BLESS",
            found.len(),
            deleted.join(", ")
        ));
    }
    Ok(())
}

const GOOD: &str = "feature start\n  show \"x\"\n";

#[test]
fn blessing_writes_the_svgs_deletes_stale_ones_and_still_fails() {
    let dir = TempDir::new("flowchart-bless-run");
    std::fs::write(dir.0.join("a.gloss"), GOOD).unwrap();
    std::fs::write(dir.0.join("a.gone.svg"), "old\n").unwrap();
    let found = flowchart_sweep(&dir.0, true).unwrap();
    let err = check_cases(&dir.0, &found, true).unwrap_err();
    assert!(
        err.starts_with("blessed 1 cases") && err.contains("a.gone.svg"),
        "{err}"
    );
    assert!(dir.0.join("a.start.svg").is_file() && !dir.0.join("a.gone.svg").exists());
    assert_eq!(check_cases(&dir.0, &found, false), Ok(()));
}

#[test]
fn a_case_that_does_not_check_does_not_hide_the_others() {
    let dir = TempDir::new("flowchart-bad-case");
    std::fs::write(dir.0.join("a.gloss"), GOOD).unwrap();
    std::fs::write(dir.0.join("b.gloss"), "feature start\n    show \"x\"\n").unwrap();
    std::fs::write(dir.0.join("c.gloss"), GOOD).unwrap();
    let found = flowchart_sweep(&dir.0, true).unwrap();
    let err = check_cases(&dir.0, &found, false).unwrap_err();
    assert!(err.contains("b.gloss does not check: "), "{err}");
    assert!(
        err.contains("a.start.svg") && err.contains("c.start.svg"),
        "{err}"
    );
}

#[test]
fn line_ending_differences_are_named() {
    let m = first_difference("f.svg", "a\nb", "a\nb\n").unwrap();
    assert!(m.contains("final newline"), "{m}");
    let m = first_difference("f.svg", "a\r\nb\n", "a\nb\n").unwrap();
    assert!(m.contains("line endings"), "{m}");
    let m = first_difference("f.svg", "a\n", "a\nb\n").unwrap();
    assert!(
        m.contains("line 2") && m.contains("<end of file>") && !m.contains("None"),
        "{m}"
    );
}

#[test]
fn every_flowchart_case_matches() {
    let dir = case_dir();
    let found = flowchart_sweep(&dir, blessing()).unwrap();
    assert!(
        found.len() >= MIN_FLOWCHART_CASES,
        "too few flowchart cases"
    );
    check_cases(&dir, &found, blessing()).unwrap();
}
