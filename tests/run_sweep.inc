// Shared by tests/run.rs and src/irsim/cases.rs through `include!`, so both read the
// run cases the same way. Needs `std::path::{Path, PathBuf}` in scope.

/// Raised by every task that adds cases.
const MIN_CASES: usize = 108;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss, out, in, err, status, ulimit, allow, args, links
/// (files) or files, after (folders), or no `<stem>.gloss`, or a folder where a file belongs
/// or the reverse.
fn sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut programs = Vec::new();
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let owned = path.with_extension("gloss").is_file();
        match path.extension().and_then(|x| x.to_str()) {
            Some("gloss") => programs.push(path),
            Some("files" | "after") if owned && path.is_dir() => {}
            Some("out" | "in" | "err" | "status" | "ulimit" | "links" | "allow" | "args")
                if owned && path.is_file() => {}
            _ => orphans.push(path.display().to_string()),
        }
    }
    programs.sort();
    if orphans.is_empty() {
        Ok(programs)
    } else {
        Err(format!("not part of a run case: {}", orphans.join(", ")))
    }
}

/// A folder's regular files, recursively, by relative path; a symbolic link is listed as
/// the key `link -> target` with no bytes.
type Tree = std::collections::BTreeMap<String, Vec<u8>>;

fn read_tree(root: &Path) -> Tree {
    fn walk(root: &Path, dir: &Path, out: &mut Tree) {
        for entry in std::fs::read_dir(dir).expect("the folder lists") {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().to_str().unwrap().to_string();
            let kind = std::fs::symlink_metadata(&path).unwrap().file_type();
            if kind.is_symlink() {
                let target = std::fs::read_link(&path).unwrap();
                out.insert(format!("{rel} -> {}", target.display()), Vec::new());
            } else if kind.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = Tree::new();
    walk(root, root, &mut out);
    out
}

/// The lines of the file `<case>.<ext>` beside a case; none when it is absent.
fn case_lines(case: &Path, ext: &str) -> Vec<String> {
    let text = std::fs::read_to_string(case.with_extension(ext)).unwrap_or_default();
    text.lines().map(String::from).collect()
}

/// The files the program's folder must hold after the run: `.after/` (else `.files/`) plus the
/// links of `.links`, each listed as `link -> target`.
fn expected_tree(case: &Path) -> Tree {
    let after = case.with_extension("after");
    let from = if after.is_dir() { after } else { case.with_extension("files") };
    let mut want = if from.is_dir() { read_tree(&from) } else { Tree::new() };
    want.extend(case_lines(case, "links").into_iter().map(|l| (l, Vec::new())));
    want
}

/// `Ok` when the trees are equal; otherwise a message naming every extra, missing and
/// different file.
fn compare_trees(got: &Tree, want: &Tree) -> Result<(), String> {
    let mut problems = Vec::new();
    for (name, bytes) in got {
        match want.get(name) {
            None => problems.push(format!("extra: {name}")),
            Some(w) if w != bytes => problems.push(format!("different bytes: {name}")),
            Some(_) => {}
        }
    }
    problems.extend(
        want.keys()
            .filter(|n| !got.contains_key(*n))
            .map(|n| format!("missing: {n}")),
    );
    if problems.is_empty() { Ok(()) } else { Err(problems.join("; ")) }
}
