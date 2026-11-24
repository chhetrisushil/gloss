//! The `tests/run` cases, run through `lower` and `irsim`: the real executable must agree.

use std::path::{Path, PathBuf};

use crate::check::check;
use crate::irsim::{Faults, Files, run_with};
use crate::lower::lower;
use crate::source::Source;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run_sweep.inc"));

/// The name of the link a `.links` line `link -> target` makes.
fn link_name(line: &str) -> &str {
    line.split_once(" -> ").expect("a `link -> target` line").0
}

/// What the program left behind, listed like `read_tree` lists a real folder: the files, and
/// each link as its `.links` line; a link the `.links` file does not give is `name -> ?`.
/// `Files` keeps link names, not targets, so a link renamed over another listed link would
/// show the old target; nothing in the runtime makes or moves links, so no case can do that.
fn left_behind(files: &Files, links: &[String]) -> Tree {
    let mut tree: Tree = files.files.clone();
    for name in &files.links {
        let line = links.iter().find(|l| link_name(l) == name);
        tree.insert(line.cloned().unwrap_or(format!("{name} -> ?")), Vec::new());
    }
    tree
}

/// The faults that stand for `.mode` lines. Linux checks the owner's bits (the runner owns the
/// files): a file without 0o400 is refused by `Open` (one such file or folder per case); a
/// folder without 0o200 refuses to create files; a folder without 0o100 refuses every path
/// through it, which is shown by refusing the first file under it. A file may lose its write
/// bit freely: the runtime replaces a table by renaming a new file over it.
fn mode_faults(files: &Files, modes: &[(String, u32)]) -> Faults {
    let mut faults = Faults::default();
    let refuse = |faults: &mut Faults, path: &str| {
        assert!(faults.failing_open.is_none(), "one refused path per case");
        faults.failing_open = Some(path.to_string());
    };
    for (path, mode) in modes {
        if files.files.contains_key(path) {
            if mode & 0o400 == 0 {
                refuse(&mut faults, path);
            }
        } else if files.folders.contains(path) {
            if mode & 0o200 == 0 {
                faults.read_only_folders.insert(path.clone());
            }
            if mode & 0o100 == 0 {
                let inside = format!("{path}/");
                let first = files.files.keys().find(|f| f.starts_with(&inside));
                refuse(&mut faults, first.expect("a file inside the closed folder"));
            }
        } else {
            panic!("{path}: the .mode path is neither a seeded file nor a folder");
        }
    }
    faults
}

#[test]
fn a_stray_file_or_link_is_reported() {
    let links = ["l -> a".to_string()];
    let mut files = Files::default();
    files.files.insert("a".into(), b"1".to_vec());
    files.links.insert("l".into());
    let want = left_behind(&files, &links);
    assert_eq!(compare_trees(&left_behind(&files, &links), &want), Ok(()));
    files.files.insert("a.gloss-new".into(), b"x".to_vec());
    let err = compare_trees(&left_behind(&files, &links), &want);
    assert_eq!(err, Err("extra: a.gloss-new".to_string()));
    files.files.remove("a.gloss-new");
    files.links.insert("m".into());
    let err = compare_trees(&left_behind(&files, &links), &want);
    assert_eq!(err, Err("extra: m -> ?".to_string()));
}

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
        // The seeded files, the links (a link's target is never reached in irsim, so it is
        // remembered here to list the link as `link -> target` like the real run) and the args.
        let mut files = Files::default();
        let seeded = case.with_extension("files");
        if seeded.is_dir() {
            for (path, bytes) in read_tree(&seeded) {
                for (i, _) in path.match_indices('/') {
                    files.folders.insert(path[..i].to_string());
                }
                files.files.insert(path, bytes);
            }
        }
        let targets: Vec<String> = case_lines(&case, "links");
        files.links = targets.iter().map(|l| link_name(l).to_string()).collect();
        let args = case_lines(&case, "args");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let input = beside("in").unwrap_or_default();
        let faults = mode_faults(&files, &case_modes(&case));
        let o = run_with(&module, "_start", &input, &args, faults, &mut files);
        let left = left_behind(&o.files, &targets);
        if let Err(problem) = compare_trees(&left, &expected_tree(&case)) {
            panic!("{case:?}: the files differ: {problem}");
        }
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
