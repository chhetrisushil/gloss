//! Every `examples/<name>.gloss` is built by the real `gloss` binary and run on Linux, and its
//! output must equal `<name>.out` beside it. Each example must also already be in the layout
//! `gloss format` writes, so the examples read as canonical Gloss.

mod run_support;

use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{TempDir, run};

/// Raised by every change that adds examples, so a lost file fails the sweep.
const MIN_EXAMPLES: usize = 7;

fn examples() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "gloss"))
        .collect();
    found.sort();
    found
}

fn gloss(args: &[&str], file: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gloss"))
        .args(args)
        .arg(file)
        .output()
        .unwrap()
}

#[test]
fn every_example_builds_runs_and_shows_its_output() {
    let found = examples();
    assert!(found.len() >= MIN_EXAMPLES, "too few examples");
    for example in found {
        let name = example.file_stem().unwrap().to_str().unwrap();
        let dir = TempDir::new(&format!("example-{name}"));
        let source = dir.0.join(format!("{name}.gloss"));
        std::fs::copy(&example, &source).unwrap();
        let built = gloss(&["build"], &source);
        assert_eq!(built.status.code(), Some(0), "{name}: {built:?}");
        let Some(ran) = run(&dir.0.join(name), b"") else {
            continue;
        };
        let want = std::fs::read(example.with_extension("out")).expect("every example has an .out");
        assert_eq!(ran.unwrap(), (0, want, Vec::new()), "{name}");
    }
}

#[test]
fn every_example_is_already_formatted() {
    for example in examples() {
        let name = example.file_stem().unwrap().to_str().unwrap();
        let dir = TempDir::new(&format!("format-{name}"));
        let copy = dir.0.join(format!("{name}.gloss"));
        std::fs::copy(&example, &copy).unwrap();
        let formatted = gloss(&["format"], &copy);
        assert_eq!(formatted.status.code(), Some(0), "{name}: {formatted:?}");
        let before = std::fs::read(&example).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            before,
            "{name} is not formatted"
        );
    }
}
