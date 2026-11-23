//! Build-time approval of the files a program can touch (D25, D27, D28).

use std::io::Write;
use std::path::Path;

use crate::cli::{Prompt, write_atomically};
use crate::permissions::{self, Entry};

/// P3: every `--allow` must name a file some table of the program uses, exactly as written.
pub fn check_flags(entries: &[Entry], flags: &[String], err: &mut dyn Write) -> Result<(), i32> {
    for flag in flags {
        if !entries.iter().any(|e| e.file == *flag) {
            let shown = permissions::display_name(flag);
            let _ = writeln!(err, "--allow {flag}: this program has no table in {shown}.");
            return Err(3);
        }
    }
    Ok(())
}

/// Gets approval for `entries` from the `.allow` beside `source`, the `--allow` flags or the
/// person at the terminal, then makes `.allow` hold exactly `entries`. `Err(status)` stops the
/// build: 3 for a file problem, 4 for no approval. Nothing is written without approval.
pub fn approve(
    source: &Path,
    name: &str,
    entries: &[Entry],
    flags: &[String],
    prompt: &mut dyn Prompt,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), i32> {
    let path = source.with_extension("allow");
    let label = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    let existing = match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(_) => {
                let _ = writeln!(err, "cannot read {label}: it is not UTF-8 text");
                return Err(3);
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            let _ = writeln!(err, "cannot read {label}: {e}");
            return Err(3);
        }
    };
    let recorded = permissions::parse_allow(existing.as_deref().unwrap_or("")).map_err(
        |(line, problem)| {
            let _ = writeln!(err, "{label} line {line}: {problem}.");
            3
        },
    )?;
    let needing = permissions::needing_approval(entries, &recorded, flags);
    if let Some(first) = needing.first() {
        if !prompt.is_terminal() {
            let names: Vec<String> = needing
                .iter()
                .map(|e| permissions::display_name(&e.file))
                .collect();
            let _ = writeln!(
                err,
                "Not approved: {}. Build in a terminal to answer, or add --allow {}.",
                names.join(", "),
                shell_word(&first.file)
            );
            return Err(4);
        }
        let answer = prompt.ask("Allow? ").unwrap_or_default();
        if !matches!(answer.trim().to_lowercase().as_str(), "yes" | "y") {
            let _ = writeln!(err, "Not approved.");
            return Err(4);
        }
    }
    let wanted = permissions::allow_file(name, entries);
    if existing.as_deref() != Some(wanted.as_str()) {
        // `.allow` records the approval, not the build, so it is saved before lowering.
        let saved = replaceable(&path).and_then(|()| {
            write_atomically(&path, wanted.as_bytes(), Some(0o644)).map_err(|e| e.to_string())
        });
        if let Err(e) = saved {
            let _ = writeln!(err, "cannot write {label}: {e}");
            return Err(3);
        }
        let _ = writeln!(out, "Approval saved in {label}");
    }
    Ok(())
}

/// Refuses to replace an `.allow` that is a symbolic link, not a regular file or read-only.
fn replaceable(path: &Path) -> Result<(), String> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() {
        Err("it is a symbolic link".into())
    } else if !meta.is_file() {
        Err("it is not a regular file".into())
    } else if meta.permissions().readonly() {
        Err("it is read-only".into())
    } else {
        Ok(())
    }
}

/// `name` as one shell word: as is when it is plain, else in single quotes.
fn shell_word(name: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "._/-".contains(c);
    if !name.is_empty() && name.chars().all(plain) {
        return name.to_string();
    }
    format!("'{}'", name.replace('\'', "'\\''"))
}
