// The explain cases, read by tests/explain_cases.rs through `include!`. Needs
// `std::path::{Path, PathBuf}` in scope.

/// Raised by every change that adds cases.
const MIN_EXPLAIN_CASES: usize = 7;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss and out, no `<stem>.gloss`, or a folder. A `.gloss`
/// with no `.out` is also an error, naming the program.
fn explain_sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut programs = Vec::new();
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let owned = path.with_extension("gloss").is_file();
        match path.extension().and_then(|x| x.to_str()) {
            Some("gloss") if path.is_file() => {
                if !path.with_extension("out").is_file() {
                    orphans.push(format!("{} has no .out", path.display()));
                }
                programs.push(path);
            }
            Some("out") if owned && path.is_file() => {}
            _ => orphans.push(path.display().to_string()),
        }
    }
    programs.sort();
    if orphans.is_empty() {
        Ok(programs)
    } else {
        Err(format!("not part of an explain case: {}", orphans.join(", ")))
    }
}
