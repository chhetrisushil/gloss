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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::Access;
    use std::path::PathBuf;

    struct Fake {
        terminal: bool,
        answer: Option<String>,
        asked: Vec<String>,
    }

    impl Fake {
        fn new(terminal: bool, answer: Option<&str>) -> Fake {
            Fake {
                terminal,
                answer: answer.map(str::to_string),
                asked: Vec::new(),
            }
        }
    }

    impl Prompt for Fake {
        fn is_terminal(&self) -> bool {
            self.terminal
        }
        fn ask(&mut self, question: &str) -> Option<String> {
            self.asked.push(question.to_string());
            self.answer.clone()
        }
    }

    fn entry(file: &str) -> Entry {
        Entry {
            file: file.to_string(),
            access: Access::ReadOnly,
        }
    }

    /// A fresh folder for this test; `t.gloss` is the source that need not exist.
    fn folder() -> PathBuf {
        let name = std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-");
        let dir = std::env::temp_dir().join(format!("gloss-approve-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct Ran {
        result: Result<(), i32>,
        out: String,
        err: String,
    }

    fn approve_in(dir: &Path, files: &[&str], flags: &[&str], prompt: &mut Fake) -> Ran {
        let flags: Vec<String> = flags.iter().map(|s| s.to_string()).collect();
        let entries: Vec<Entry> = files.iter().map(|f| entry(f)).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let source = dir.join("t.gloss");
        let result = approve(
            &source, "t.gloss", &entries, &flags, prompt, &mut out, &mut err,
        );
        Ran {
            result,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    /// Runs `approve` for `a.csv` read only with `allow` as `t.allow`; returns the run and the
    /// `.allow` text afterwards.
    fn go(allow: Option<&str>, flags: &[&str], prompt: &mut Fake) -> (Ran, Option<String>) {
        let dir = folder();
        if let Some(text) = allow {
            std::fs::write(dir.join("t.allow"), text).unwrap();
        }
        let ran = approve_in(&dir, &["a.csv"], flags, prompt);
        let after = std::fs::read_to_string(dir.join("t.allow")).ok();
        let _ = std::fs::remove_dir_all(&dir);
        (ran, after)
    }

    const APPROVED: &str =
        "note Files this program may touch; approved for t.gloss.\n  read only          a.csv\n";

    fn approves_with(answer: &str) {
        let mut p = Fake::new(true, Some(answer));
        let (ran, after) = go(None, &[], &mut p);
        assert_eq!(ran.result, Ok(()), "{answer:?}");
        assert_eq!(p.asked, ["Allow? "]);
        assert_eq!(ran.out, "Approval saved in t.allow\n");
        assert_eq!(after.as_deref(), Some(APPROVED));
    }

    #[test]
    fn yes_approves() {
        approves_with("yes");
        approves_with("  yes \n");
    }

    #[test]
    fn y_in_capitals_approves() {
        approves_with("Y");
        approves_with("YES");
    }

    #[test]
    fn no_refuses_with_status_4() {
        for answer in ["no", "", "yess", "yes please"] {
            let mut p = Fake::new(true, Some(answer));
            let (ran, after) = go(None, &[], &mut p);
            assert_eq!((ran.result, ran.out.as_str()), (Err(4), ""));
            assert_eq!(ran.err, "Not approved.\n");
            assert_eq!(after, None);
        }
    }

    #[test]
    fn end_of_input_refuses() {
        let mut p = Fake::new(true, None);
        let (ran, after) = go(None, &[], &mut p);
        assert_eq!(
            (ran.result, ran.err.as_str(), after),
            (Err(4), "Not approved.\n", None)
        );
    }

    #[test]
    fn the_question_is_not_asked_when_covered() {
        let mut p = Fake::new(true, Some("no"));
        let (ran, after) = go(None, &["a.csv"], &mut p);
        assert_eq!((ran.result, after.as_deref()), (Ok(()), Some(APPROVED)));
        assert!(p.asked.is_empty());
        let (ran, _) = go(Some(APPROVED), &[], &mut p);
        assert_eq!((ran.result, ran.out.as_str()), (Ok(()), ""));
        assert!(p.asked.is_empty());
    }

    #[test]
    fn a_flag_covering_some_files_leaves_the_question_for_the_rest() {
        let dir = folder();
        let mut p = Fake::new(true, Some("y"));
        let ran = approve_in(&dir, &["a.csv", "b.csv"], &["a.csv"], &mut p);
        assert_eq!(ran.result, Ok(()));
        assert_eq!(p.asked, ["Allow? "]);
        let text = std::fs::read_to_string(dir.join("t.allow")).unwrap();
        assert!(text.contains("a.csv") && text.contains("b.csv"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn not_a_terminal_refuses_without_asking_and_leaves_allow_alone() {
        let mut p = Fake::new(false, Some("yes"));
        let (ran, after) = go(None, &[], &mut p);
        assert_eq!((ran.result, ran.out.as_str(), after), (Err(4), "", None));
        assert_eq!(
            ran.err,
            "Not approved: a.csv. Build in a terminal to answer, or add --allow a.csv.\n"
        );
        let old = "note old\n  read only          z.csv\n";
        let (ran, after) = go(Some(old), &[], &mut p);
        assert_eq!((ran.result, after.as_deref()), (Err(4), Some(old)));
        assert!(p.asked.is_empty());
    }
}
