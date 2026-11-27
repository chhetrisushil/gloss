// The flowchart cases, read by tests/flowchart_cases.rs through `include!`. Needs
// `std::path::{Path, PathBuf}` in scope.

/// Raised by every change that adds cases.
const MIN_FLOWCHART_CASES: usize = 4;

/// The `.gloss` programs in `dir`, sorted; an error naming every file that is not part of a
/// case: an extension other than gloss and svg, a `.svg` that is not `<case>.<feature>.svg`
/// beside an existing `<case>.gloss`, or a folder. A `.gloss` with no `.svg` beside it is also
/// an error, naming the program, unless `bless` is set (the run then creates them). Dotfiles
/// such as `.DS_Store` are ignored.
fn flowchart_sweep(dir: &Path, bless: bool) -> Result<Vec<PathBuf>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.file_name().is_some_and(|n| n.as_encoded_bytes().starts_with(b".")) {
            continue;
        }
        entries.push(path);
    }
    let mut programs = Vec::new();
    let mut orphans = Vec::new();
    for path in &entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !path.is_file() {
            orphans.push(path.display().to_string());
        } else if name.ends_with(".gloss") {
            let stem = name.trim_end_matches(".gloss");
            let prefix = format!("{stem}.");
            let has_svg = entries.iter().any(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".svg"))
            });
            if !bless && !has_svg {
                orphans.push(format!("{} has no .svg", path.display()));
            }
            programs.push(path.clone());
        } else if let Some(stem) = name.strip_suffix(".svg").and_then(|s| s.split('.').next())
            && !stem.is_empty()
            && name != format!("{stem}.svg")
            && dir.join(format!("{stem}.gloss")).is_file()
        {
        } else {
            orphans.push(path.display().to_string());
        }
    }
    programs.sort();
    if orphans.is_empty() {
        Ok(programs)
    } else {
        orphans.sort();
        Err(format!("not part of a flowchart case: {}", orphans.join(", ")))
    }
}
