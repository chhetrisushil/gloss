// Shared by tests/trace_cases.rs and src/irsim/trace_cases.rs through `include!`, so both read
// the trace cases the same way. Needs `std::path::{Path, PathBuf}` in scope.

/// Raised by every task that adds cases.
const MIN_TRACE_CASES: usize = 6;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss, trace, out, in, err, status (files) or files (a
/// folder), no `<stem>.gloss`, or a folder where a file belongs or the reverse. A `.gloss`
/// with no `.trace` or no `.out` is also an error, naming the program.
fn trace_sweep(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut programs = Vec::new();
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let owned = path.with_extension("gloss").is_file();
        match path.extension().and_then(|x| x.to_str()) {
            Some("gloss") if path.is_file() => {
                for needed in ["trace", "out"] {
                    if !path.with_extension(needed).is_file() {
                        orphans.push(format!("{} has no .{needed}", path.display()));
                    }
                }
                programs.push(path);
            }
            Some("files") if owned && path.is_dir() => {}
            Some("trace" | "out" | "in" | "err" | "status") if owned && path.is_file() => {}
            _ => orphans.push(path.display().to_string()),
        }
    }
    programs.sort();
    if orphans.is_empty() {
        Ok(programs)
    } else {
        Err(format!("not part of a trace case: {}", orphans.join(", ")))
    }
}
