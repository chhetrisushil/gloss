//! Every `tests/run/<name>.gloss` is built by the real `gloss` binary and run on Linux.
//! Beside it: `.out` (required), and optional `.in`, `.err`, `.status` and `.ulimit` (a
//! virtual-memory limit in KiB, applied with `ulimit -v` in the shell that runs the program),
//! `.args` (one argument per line), `.allow` (copied beside the source), `.files/` (seeded into
//! the program's current folder, `work/`), `.links` (`link -> target` lines, made inside
//! `work/`), `.mode` (`path octal-mode` lines, applied to `work/` after seeding, deepest path
//! first; skipped, and counted, when the runner's user is root), `.after_mode` (`path
//! octal-mode` lines: the permission bits those paths must have when the program ends) and `.after/` (the files that
//! must be left in `work/`, besides the links; without it `work/` must end up as `.files/`
//! was).

mod run_support;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use run_support::{DEADLINE, TempDir, copy_tree, run_case, run_env, runner, runs_as_root};

include!("run_sweep.inc");

fn cases() -> Vec<PathBuf> {
    sweep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run")).unwrap()
}

fn beside(case: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(case.with_extension(ext)).ok()
}

#[test]
fn the_sweep_finds_the_run_cases() {
    assert!(cases().len() >= MIN_CASES, "too few run cases");
}

#[test]
fn the_sweep_reports_an_orphan() {
    let dir = TempDir::new("sweep");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    std::fs::write(dir.0.join("a.out"), "").unwrap();
    assert_eq!(sweep(&dir.0).unwrap().len(), 1);
    for orphan in [
        "x.out",
        "b.GLOSS",
        "a.txt",
        "x.ulimit",
        "x.files",
        "y.links",
        "x.after",
        "x.allow",
        "x.args",
        "x.mode",
        "x.after_mode",
    ] {
        std::fs::write(dir.0.join(orphan), "").unwrap();
        let err = sweep(&dir.0).unwrap_err();
        assert!(err.contains(orphan), "{err}");
        std::fs::remove_file(dir.0.join(orphan)).unwrap();
    }
}

#[test]
fn the_sweep_accepts_case_folders_and_rejects_plain_files_for_them() {
    let dir = TempDir::new("sweep-folders");
    std::fs::write(dir.0.join("a.gloss"), "").unwrap();
    for file in [
        "a.out",
        "a.args",
        "a.allow",
        "a.links",
        "a.mode",
        "a.after_mode",
    ] {
        std::fs::write(dir.0.join(file), "").unwrap();
    }
    for folder in ["a.files", "a.after"] {
        std::fs::create_dir(dir.0.join(folder)).unwrap();
    }
    assert_eq!(sweep(&dir.0).unwrap().len(), 1);
    for folder in ["a.files", "a.after"] {
        std::fs::remove_dir(dir.0.join(folder)).unwrap();
        std::fs::write(dir.0.join(folder), "").unwrap();
        let err = sweep(&dir.0).unwrap_err();
        assert!(err.contains(folder), "{err}");
    }
}

fn tree_of(files: &[(&str, &str)]) -> Tree {
    files
        .iter()
        .map(|(n, b)| (n.to_string(), b.as_bytes().to_vec()))
        .collect()
}

#[test]
fn the_folder_comparison_reports_every_difference() {
    let want = tree_of(&[("a", "1"), ("d/b", "2"), ("l -> a", "")]);
    assert_eq!(compare_trees(&want.clone(), &want), Ok(()));
    // A leftover `.gloss-new` must be seen.
    let mut got = want.clone();
    got.insert("a.gloss-new".into(), b"x".to_vec());
    assert_eq!(compare_trees(&got, &want), Err("extra: a.gloss-new".into()));
    let mut got = want.clone();
    got.remove("d/b");
    assert_eq!(compare_trees(&got, &want), Err("missing: d/b".into()));
    let mut got = want.clone();
    got.insert("a".into(), b"11".to_vec());
    assert_eq!(compare_trees(&got, &want), Err("different bytes: a".into()));
    let mut got = want.clone();
    got.remove("l -> a");
    got.insert("l -> b".into(), Vec::new());
    let err = compare_trees(&got, &want).unwrap_err();
    assert!(
        err.contains("extra: l -> b") && err.contains("missing: l -> a"),
        "{err}"
    );
}

#[test]
fn the_folder_listing_sees_files_links_and_nested_folders() {
    let dir = TempDir::new("listing");
    std::fs::create_dir_all(dir.0.join("d/e")).unwrap();
    std::fs::write(dir.0.join("d/e/f"), "x").unwrap();
    std::os::unix::fs::symlink("d/e/f", dir.0.join("l")).unwrap();
    assert_eq!(
        read_tree(&dir.0),
        tree_of(&[("d/e/f", "x"), ("l -> d/e/f", "")])
    );
}

/// Applies the `.mode` lines of a case to `work`, deepest path first so a nested path is
/// reached before its folder is closed; on drop (also when a panic unwinds) gives every path
/// back a mode that can be read, listed and removed, shallowest first.
struct Modes {
    work: PathBuf,
    paths: Vec<String>,
}

impl Modes {
    /// The guard is built before the first chmod, so a failing chmod still restores.
    fn apply(work: &Path, modes: &[(String, u32)]) -> Modes {
        let depth = |p: &str| p.matches('/').count();
        let mut sorted = modes.to_vec();
        sorted.sort_by_key(|(p, _)| std::cmp::Reverse(depth(p)));
        let guard = Modes {
            work: work.to_path_buf(),
            paths: sorted.iter().map(|(p, _)| p.clone()).collect(),
        };
        for (path, mode) in &sorted {
            let mode = std::fs::Permissions::from_mode(*mode);
            std::fs::set_permissions(work.join(path), mode).unwrap();
        }
        guard
    }
}

impl Drop for Modes {
    fn drop(&mut self) {
        for path in self.paths.iter().rev() {
            let path = self.work.join(path);
            let mode = if path.is_dir() { 0o755 } else { 0o644 };
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
        }
    }
}

#[test]
fn the_mode_lines_are_paths_with_octal_modes() {
    let dir = TempDir::new("modes");
    let case = dir.0.join("a.gloss");
    std::fs::write(case.with_extension("mode"), "d e.csv 000\nd 500\n").unwrap();
    let want = [("d e.csv".to_string(), 0), ("d".to_string(), 0o500)];
    assert_eq!(case_modes(&case), want);
}

#[test]
fn nested_modes_apply_deepest_first_and_dropping_the_guard_restores_them() {
    for lines in [
        [("d".to_string(), 0), ("d/e.csv".to_string(), 0)],
        [("d/e.csv".to_string(), 0), ("d".to_string(), 0)],
    ] {
        let dir = TempDir::new("nested-modes");
        std::fs::create_dir(dir.0.join("d")).unwrap();
        std::fs::write(dir.0.join("d/e.csv"), "x").unwrap();
        let guard = Modes::apply(&dir.0, &lines);
        let closed = std::fs::metadata(dir.0.join("d")).unwrap().permissions();
        assert_eq!(closed.mode() & 0o777, 0);
        drop(guard);
        assert_eq!(std::fs::read(dir.0.join("d/e.csv")).unwrap(), b"x");
    }
}

#[test]
fn a_panic_leaves_the_modes_restored() {
    let dir = TempDir::new("panic-modes");
    std::fs::write(dir.0.join("f"), "x").unwrap();
    let lines = [("f".to_string(), 0)];
    let caught = std::panic::catch_unwind(|| {
        let _guard = Modes::apply(&dir.0, &lines);
        panic!("a run fails while the modes are set");
    });
    assert!(caught.is_err());
    assert_eq!(std::fs::read(dir.0.join("f")).unwrap(), b"x");
}

#[test]
fn every_run_case_builds_and_runs() {
    let (mut ran, mut skipped) = (0, 0);
    let wanted = cases()
        .iter()
        .filter(|c| c.with_extension("mode").is_file())
        .count();
    for case in cases() {
        let name = case.file_stem().unwrap().to_str().unwrap();
        let dir = TempDir::new(&format!("run-{name}"));
        let source = dir.0.join(format!("{name}.gloss"));
        std::fs::copy(&case, &source).unwrap();
        // The build reads `<source>.allow`, so it must be in place first.
        if case.with_extension("allow").is_file() {
            std::fs::copy(case.with_extension("allow"), source.with_extension("allow")).unwrap();
        }
        let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
            .arg("build")
            .arg(&source)
            .output()
            .unwrap();
        assert_eq!(built.status.code(), Some(0), "{name}: {built:?}");
        let kib = beside(&case, "ulimit").map(|k| {
            let text = String::from_utf8(k).unwrap();
            text.trim()
                .parse::<u64>()
                .unwrap_or_else(|e| panic!("{name}.ulimit: {e}"))
        });
        let input = beside(&case, "in").unwrap_or_default();
        // The program runs in its own folder, `work/`, beside the executable.
        let work = dir.0.join("work");
        std::fs::create_dir_all(&work).unwrap();
        if case.with_extension("files").is_dir() {
            copy_tree(&case.with_extension("files"), &work);
        }
        for link in case_lines(&case, "links") {
            let (name, target) = link.split_once(" -> ").expect("a `link -> target` line");
            std::os::unix::fs::symlink(target, work.join(name)).unwrap();
        }
        let args = case_lines(&case, "args");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let modes = case_modes(&case);
        // Root ignores permission bits, so the program would not fail: skip, visibly.
        if !modes.is_empty() && runner().is_some_and(|p| runs_as_root(&p)) {
            skipped += 1;
            let _ = writeln!(std::io::stderr(), "SKIP {name}: the runner's user is root");
            continue;
        }
        let guard = Modes::apply(&work, &modes);
        let result = run_case(&dir.0.join(name), &input, DEADLINE, &work, &args, kib);
        // Read before the guard gives every path back a readable mode.
        let kept: Vec<(String, u32)> = case_after_modes(&case)
            .into_iter()
            .map(|(path, _)| {
                let mode = std::fs::metadata(work.join(&path))
                    .map_or(0, |m| m.permissions().mode() & 0o777);
                (path, mode)
            })
            .collect();
        // Restored before the folder is read; the guard also restores if the run panics.
        drop(guard);
        let Some(result) = result else {
            continue;
        };
        let status = beside(&case, "status")
            .map_or(0, |s| String::from_utf8(s).unwrap().trim().parse().unwrap());
        let want = (
            status,
            beside(&case, "out").expect("every case has an .out file"),
            beside(&case, "err").unwrap_or_default(),
        );
        assert_eq!(result.unwrap(), want, "{name}");
        if let Err(problem) = compare_trees(&read_tree(&work), &expected_tree(&case)) {
            panic!("{name}: the folder differs: {problem}");
        }
        assert_eq!(
            kept,
            case_after_modes(&case),
            "{name}: permissions after the run"
        );
        if !modes.is_empty() {
            ran += 1;
            let _ = writeln!(std::io::stderr(), "PASS {name}: ran with modes {modes:?}");
        }
    }
    // Without a runner every case is skipped; otherwise a permission case ran or was counted.
    if runner().is_some() {
        let _ = writeln!(
            std::io::stderr(),
            "permission cases: {ran} ran, {skipped} skipped"
        );
        assert_eq!(ran + skipped, wanted, "a permission case vanished");
    }
}

/// A large environment sits above the stack pointer at entry; the limit must still be measured
/// from the top of the stack, so deep recursion fails with code 10 instead of faulting.
#[test]
fn too_deep_still_fails_cleanly_with_a_huge_environment() {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/too_deep.gloss");
    let dir = TempDir::new("run-big-env");
    let source = dir.0.join("too_deep.gloss");
    std::fs::copy(&case, &source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    // Four variables of 60000 bytes: a single string is limited to 128 KiB.
    let vars: Vec<(&str, String)> = ["BIG1", "BIG2", "BIG3", "BIG4"]
        .map(|k| (k, "x".repeat(60000)))
        .to_vec();
    let Some(ran) = run_env(&dir.0.join("too_deep"), b"", DEADLINE, &vars) else {
        return;
    };
    let want = (2, Vec::new(), beside(&case, "err").unwrap());
    assert_eq!(ran.unwrap(), want);
}
