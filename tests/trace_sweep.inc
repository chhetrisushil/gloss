// Shared by tests/trace_cases.rs and src/irsim/trace_cases.rs through `include!`, so both read
// the trace cases the same way. Needs `std::path::{Path, PathBuf}` in scope.

/// Raised by every task that adds cases.
const MIN_TRACE_CASES: usize = 45;

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

/// A program of two features that call each other until failure 10. Its whole trace is about
/// 100k lines, so it is not a case: tests/trace_cases.rs (Linux) and src/irsim/trace_cases.rs
/// (irsim) run it and compare `deep_ending` of its trace and the status with `DEEP_ENDING`.
const DEEP_PROGRAM: &str = "feature ping\n  needs n as number\n  set r to pong with n + 1\n  if n is -1\n    give 0\n  give r + 1\n\nfeature pong\n  needs n as number\n  set r to ping with n + 1\n  if n is -1\n    give 0\n  give r + 1\n\nfeature start\n  show (ping with 1)\n";

const DEEP_ENDING: &str = "enter of the other feature, then fail 10 of the call that ran out";

/// Describes the last two lines of the trace in `stdout`. The real stack and irsim's are
/// different sizes, so the two runs fail at different depths and at different ones of the two
/// features; what must hold is that the failing call (named in the failure) wrote no `enter`,
/// so the last `enter` is the other feature's, and the failure is at the line of the failing
/// call (3 in `ping`, which calls `pong`; 10 in `pong`). Anything else is returned as the lines.
fn deep_ending(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    let lines: Vec<&str> = text.lines().collect();
    let [enter, fail] = lines[lines.len().saturating_sub(2)..] else {
        return format!("fewer than 2 lines of trace: {text:?}");
    };
    let between = |s: &str, from: &str, to: char| -> String {
        let at = s.find(from).map_or(s.len(), |i| i + from.len());
        s[at..].split(to).next().unwrap_or("").to_string()
    };
    let entered = between(enter, "\"event\":\"enter\",\"feature\":\"", '"');
    let failing = between(fail, "calls nested too deep (", ')');
    let line = if failing == "pong" { 3 } else { 10 };
    let ok = fail.starts_with(&format!("{{\"line\":{line},\"event\":\"fail\",\"failure\":10,"))
        && ["ping", "pong"].contains(&entered.as_str())
        && ["ping", "pong"].contains(&failing.as_str())
        && entered != failing;
    if ok {
        DEEP_ENDING.to_string()
    } else {
        format!("{enter}\n{fail}")
    }
}
