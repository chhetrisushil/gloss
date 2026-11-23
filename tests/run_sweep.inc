// Shared by tests/run.rs and src/irsim/cases.rs through `include!`, so both read the
// run cases the same way. Needs `std::path::{Path, PathBuf}` in scope.

/// Raised by every task that adds cases.
const MIN_CASES: usize = 54;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss, out, in, err or status, or no `<stem>.gloss`.
fn sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut programs = Vec::new();
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        match path.extension().and_then(|x| x.to_str()) {
            Some("gloss") => programs.push(path),
            Some("out" | "in" | "err" | "status") if path.with_extension("gloss").is_file() => {}
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
