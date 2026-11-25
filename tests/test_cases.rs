//! Every `tests/test_cases/<name>.gloss` is built with `build_test_executable` and run as a
//! real executable. Beside it: `.out` (required), and optional `.err` and `.status`.

mod run_support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use run_support::{DEADLINE, TempDir, run_within, skip};

include!("test_sweep.inc");

fn cases() -> Vec<PathBuf> {
    test_sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_cases")).unwrap()
}

#[test]
fn the_sweep_finds_the_test_cases() {
    assert!(cases().len() >= MIN_TEST_CASES, "too few test cases");
}

#[test]
fn the_test_sweep_reports_an_orphan() {
    let dir = TempDir::new("test-sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.out"), "").unwrap();
    assert_eq!(test_sweep(&dir.0).unwrap().len(), 1);
    for orphan in [
        "x.out", "b.GLOSS", "a.txt", "x.err", "x.status", "a.in", "a.files",
    ] {
        std::fs::write(dir.0.join(orphan), "").unwrap();
        let err = test_sweep(&dir.0).unwrap_err();
        assert!(err.contains(orphan), "{err}");
        std::fs::remove_file(dir.0.join(orphan)).unwrap();
    }
}

#[test]
fn a_case_without_an_out_file_is_reported() {
    let dir = TempDir::new("test-sweep-no-out");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    let err = test_sweep(&dir.0).unwrap_err();
    assert!(err.contains("a.gloss"), "{err}");
}

#[test]
fn a_folder_named_like_a_case_file_is_reported() {
    let dir = TempDir::new("test-sweep-folder");
    std::fs::write(dir.0.join("x.gloss"), "").unwrap();
    std::fs::write(dir.0.join("x.out"), "").unwrap();
    assert_eq!(test_sweep(&dir.0).unwrap().len(), 1);
    for name in ["x.out", "x.err", "x.status"] {
        let folder = dir.0.join(name);
        let _ = std::fs::remove_file(&folder);
        std::fs::create_dir(&folder).unwrap();
        let err = test_sweep(&dir.0).unwrap_err();
        assert!(err.contains(name), "{err}");
        std::fs::remove_dir(&folder).unwrap();
        std::fs::write(dir.0.join("x.out"), "").unwrap();
    }
}

fn beside(case: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(case.with_extension(ext)).ok()
}

#[test]
fn every_test_case_gives_the_expected_output() {
    let dir = TempDir::new("test-cases");
    for case in cases() {
        let name = case.file_stem().unwrap().to_str().unwrap();
        let text = std::fs::read_to_string(&case).unwrap();
        let bytes = gloss::build_test_executable(&text)
            .unwrap_or_else(|d| panic!("{name}: does not build: {d:?}"));
        let exe = dir.0.join(name);
        std::fs::write(&exe, bytes).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let Some(ran) = run_within(&exe, b"", DEADLINE) else {
            skip();
            return;
        };
        let status = beside(&case, "status")
            .map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
        let want = (
            status,
            beside(&case, "out").expect("every case has an .out file"),
            beside(&case, "err").unwrap_or_default(),
        );
        assert_eq!(ran.unwrap(), want, "{name}");
    }
}
