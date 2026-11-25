//! Every `examples/<name>.gloss` is built by the real `gloss` binary and run on Linux, and its
//! output must equal `<name>.out` beside it. Each runs in a temporary copy of `examples/` as its
//! current folder, so a program that saves never changes the committed files. Each example must
//! also already be in the layout `gloss format` writes, so the examples read as canonical Gloss.

mod run_support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{DEADLINE, TempDir, run_case, skip};

/// Raised by every change that adds examples, so a lost file fails the sweep.
const MIN_EXAMPLES: usize = 11;

/// Raised by every change that gives an example tests.
const MIN_EXAMPLES_WITH_TESTS: usize = 4;

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

/// The names of the files in `examples/` and their contents, to check a run changed none.
fn snapshot() -> Vec<(PathBuf, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| (p.clone(), std::fs::read(&p).unwrap()))
        .collect();
    files.sort();
    files
}

#[test]
fn every_example_builds_runs_and_shows_its_output() {
    let found = examples();
    assert!(found.len() >= MIN_EXAMPLES, "too few examples");
    let before = snapshot();
    let folder = TempDir::new("examples");
    // Every file of `examples/` (sources, approvals, data) is copied beside the executables.
    for (path, bytes) in &before {
        std::fs::write(folder.0.join(path.file_name().unwrap()), bytes).unwrap();
    }
    for example in found {
        let name = example.file_stem().unwrap().to_str().unwrap();
        let source = folder.0.join(format!("{name}.gloss"));
        let built = gloss(&["build"], &source);
        assert_eq!(built.status.code(), Some(0), "{name}: {built:?}");
        // The build rewrites a changed `.allow` (D28), so compare it with the committed one: a
        // stale or over-broad approval must fail here.
        if let Ok(committed) = std::fs::read(example.with_extension("allow")) {
            let rebuilt = std::fs::read(source.with_extension("allow")).unwrap();
            assert_eq!(
                rebuilt, committed,
                "{name}.allow is not what the build writes"
            );
        }
        let Some(ran) = run_case(&folder.0.join(name), b"", DEADLINE, &folder.0, &[], None) else {
            continue;
        };
        let want = std::fs::read(example.with_extension("out")).expect("every example has an .out");
        assert_eq!(ran.unwrap(), (0, want, Vec::new()), "{name}");
    }
    // A tripwire for harness regressions (such as running in the real folder), not a check on
    // programs: RESOLVE_BENEATH and the temporary folder already keep a save out of `examples/`.
    assert!(
        snapshot() == before,
        "running the examples changed the committed examples/ folder"
    );
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

fn test_outputs() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_str().is_some_and(|s| s.ends_with(".tests.out")))
        .collect();
    found.sort();
    found
}

/// The example a `<name>.tests.out` belongs to.
fn program_of(tests_out: &Path) -> PathBuf {
    let name = tests_out.file_name().unwrap().to_str().unwrap();
    tests_out.with_file_name(format!(
        "{}.gloss",
        name.strip_suffix(".tests.out").unwrap()
    ))
}

#[test]
fn every_tests_out_has_its_example() {
    for out in test_outputs() {
        assert!(
            program_of(&out).is_file(),
            "{} has no .gloss",
            out.display()
        );
    }
}

/// Runs each example's tests through `build_test_executable` and the runner, not through
/// `gloss test`, which refuses to run on a host that is not Linux (D38). The folder the tests
/// run in is empty: tests use pretend files, so none is created.
#[test]
fn every_example_with_tests_passes_them() {
    let outs = test_outputs();
    assert!(
        outs.len() >= MIN_EXAMPLES_WITH_TESTS,
        "too few examples with tests"
    );
    let folder = TempDir::new("example-tests");
    for out in outs {
        let program = program_of(&out);
        let name = program.file_stem().unwrap().to_str().unwrap();
        let text = std::fs::read_to_string(&program).unwrap();
        let bytes = gloss::build_test_executable(&text)
            .unwrap_or_else(|d| panic!("{name}: does not build as tests: {d:?}"));
        let exe = folder.0.join(format!("{name}.tests"));
        std::fs::write(&exe, bytes).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let Some(ran) = run_case(&exe, b"", DEADLINE, &folder.0, &[], None) else {
            skip();
            return;
        };
        let want = std::fs::read(&out).unwrap();
        assert_eq!(ran.unwrap(), (0, want, Vec::new()), "{name}");
    }
    let left: Vec<_> = std::fs::read_dir(&folder.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(
        left.iter().all(|f| f.to_str().unwrap().ends_with(".tests")),
        "a test run left files behind: {left:?}"
    );
}
