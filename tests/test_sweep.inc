// Shared by tests/test_cases.rs and src/irsim/test_cases.rs through `include!`, so both read
// the test cases the same way. Needs `std::path::{Path, PathBuf}` in scope. It repeats
// `sweep` of tests/run_sweep.inc rather than sharing it: the extension sets differ.

/// Raised by every task that adds cases.
const MIN_TEST_CASES: usize = 7;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss, out, err, status, or no `<stem>.gloss`, or a folder
/// where a file belongs. A `.gloss` with no `.out` is also an error, naming the program.
fn test_sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
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
            Some("out" | "err" | "status") if owned && path.is_file() => {}
            _ => orphans.push(path.display().to_string()),
        }
    }
    programs.sort();
    if orphans.is_empty() {
        Ok(programs)
    } else {
        Err(format!("not part of a test case: {}", orphans.join(", ")))
    }
}
