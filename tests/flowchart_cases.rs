//! Every `tests/flowchart_cases/<case>.gloss` is charted, and each `(file, svg)` that
//! `Checked::flowcharts` returns must equal the file of that name beside it, `<case>.<feature>.svg`.
//! An expected `.svg` the program does not produce fails, and so does a produced one with no
//! expected file. `GLOSS_BLESS=1` rewrites and creates the `.svg` files, then fails on purpose.

use std::path::{Path, PathBuf};

include!("flowchart_sweep.inc");

fn blessing() -> bool {
    std::env::var_os("GLOSS_BLESS").is_some_and(|v| v == "1")
}

fn case_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/flowchart_cases")
}

fn charted(case: &Path) -> Vec<(String, String)> {
    let name = case.file_name().unwrap().to_str().unwrap();
    let text = std::fs::read_to_string(case).unwrap();
    let (diagnostics, checked) = gloss::check_program(&text);
    let checked = checked.unwrap_or_else(|| panic!("{name}: does not check: {diagnostics:?}"));
    checked.flowcharts(name)
}

/// The first line where `got` and `want` differ, as a message, or `None` when they are equal.
fn first_difference(file: &str, got: &str, want: &str) -> Option<String> {
    if got == want {
        return None;
    }
    let at = got
        .lines()
        .zip(want.lines())
        .position(|(g, w)| g != w)
        .unwrap_or_else(|| got.lines().count().min(want.lines().count()));
    Some(format!(
        "{file}: line {} differs\n  expected: {:?}\n  got:      {:?}",
        at + 1,
        want.lines().nth(at),
        got.lines().nth(at)
    ))
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
    let stem = case.file_stem().unwrap().to_str().unwrap();
    let prefix = format!("{stem}.");
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name.starts_with(&prefix)
            && name.ends_with(".svg")
            && !produced.iter().any(|(file, _)| *file == name)
        {
            problems.push(format!(
                "{name}: expected, but the program does not produce it"
            ));
        }
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
fn every_flowchart_case_matches() {
    let dir = case_dir();
    let found = flowchart_sweep(&dir, blessing()).unwrap();
    assert!(
        found.len() >= MIN_FLOWCHART_CASES,
        "too few flowchart cases"
    );
    let bless = blessing();
    let mut problems = Vec::new();
    for case in &found {
        let produced = charted(case);
        if bless {
            for (file, svg) in &produced {
                std::fs::write(dir.join(file), svg).unwrap();
            }
            continue;
        }
        problems.extend(compare(&dir, case, &produced));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        !bless,
        "blessed {} cases, rerun without GLOSS_BLESS",
        found.len()
    );
}
