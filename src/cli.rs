//! The `gloss` command line.

use std::io::Write;
use std::path::Path;

use crate::diag::Diagnostic;
use crate::source::Source;
use crate::{format_source, parse_source};

const USAGE: &str = "usage: gloss <command> <file.gloss>\ncommands: build, test, format, trace, explain, flowchart\noptions: --allow <file> (build, trace)\n";

/// Where questions to the person at the terminal go, and where their answers come from.
pub trait Prompt {
    /// Whether standard input is a terminal a person can answer at.
    fn is_terminal(&self) -> bool;
    /// Shows `question` and reads one line; `None` at the end of input.
    fn ask(&mut self, question: &str) -> Option<String>;
}

/// What `gloss build` was told beyond the source file.
pub struct BuildOptions {
    /// The files named by `--allow`, in the order given.
    pub allow: Vec<String>,
}

/// The commands that take `--allow` (the USAGE text names the same two).
const ALLOW_COMMANDS: [&str; 2] = ["build", "trace"];

/// The arguments after the command: the file, then any number of `--allow <file>`.
/// A name that is empty or starts with `-` is not a file name.
fn parse_args(args: &[String]) -> Option<(&str, &str, Vec<String>)> {
    let (command, file) = (args.first()?, args.get(1)?);
    let mut allow = Vec::new();
    let mut rest = args[2..].iter();
    while let Some(arg) = rest.next() {
        if arg != "--allow" || !ALLOW_COMMANDS.contains(&command.as_str()) {
            return None;
        }
        let name = rest.next()?;
        if name.is_empty() || name.starts_with('-') {
            return None;
        }
        allow.push(name.clone());
    }
    Some((command, file, allow))
}

/// Runs one command; returns the exit status (0 success, 3 anything else).
pub fn run(
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    prompt: &mut dyn Prompt,
) -> i32 {
    let Some((command, file, allow)) = parse_args(args) else {
        let _ = err.write_all(USAGE.as_bytes());
        return 3;
    };
    let milestone = match command {
        "build" | "format" => 0,
        "test" => 4,
        "trace" => 5,
        "explain" => 6,
        "flowchart" => 7,
        _ => {
            let _ = err.write_all(USAGE.as_bytes());
            return 3;
        }
    };
    if milestone != 0 {
        let _ = writeln!(
            err,
            "gloss {command} is not built yet; it arrives in milestone {milestone}"
        );
        return 3;
    }
    let name = Path::new(file)
        .file_name()
        .map_or(file.to_string(), |n| n.to_string_lossy().into());
    // The program's executable is named by dropping `.gloss`; without it the two could clash.
    if name.strip_suffix(".gloss").is_none_or(str::is_empty) {
        let _ = writeln!(err, "the source file must end in .gloss");
        return 3;
    }
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) => {
            let _ = writeln!(err, "cannot read {file}: {e}");
            return 3;
        }
    };
    if command == "build" {
        let options = BuildOptions { allow };
        build(Path::new(file), &name, &text, &options, prompt, out, err)
    } else {
        format(file, &name, &text, out, err)
    }
}

fn build(
    path: &Path,
    name: &str,
    text: &str,
    options: &BuildOptions,
    prompt: &mut dyn Prompt,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (mut diagnostics, checked) = crate::check_program(text);
    let clashes = executable_clashes(name, text, &diagnostics);
    diagnostics.extend(clashes);
    crate::diag::sort(&mut diagnostics);
    for d in &diagnostics {
        let _ = writeln!(err, "{}", d.render());
    }
    if diagnostics.iter().any(|d| d.is_error()) {
        return 3;
    }
    let entries = checked
        .as_ref()
        .map(|c| c.permissions())
        .unwrap_or_default();
    if crate::approve::check_flags(&entries, &options.allow, err).is_err() {
        return 3;
    }
    if !entries.is_empty() {
        let _ = out.write_all(crate::permissions::render(&entries).as_bytes());
        let approved =
            crate::approve::approve(path, name, &entries, &options.allow, prompt, out, err);
        if let Err(status) = approved {
            return status;
        }
    }
    let bytes = match checked.map(|c| c.executable()) {
        Some(Ok(bytes)) => bytes,
        Some(Err(d)) => {
            let _ = writeln!(err, "{}", d.render());
            return 3;
        }
        None => return 3,
    };
    let executable = path.with_extension("");
    if let Err(e) = write_atomically(&executable, &bytes, Some(0o755)) {
        let _ = writeln!(err, "cannot write {}: {e}", executable.display());
        return 3;
    }
    let _ = writeln!(out, "Built {}", name.strip_suffix(".gloss").unwrap_or(name));
    0
}

/// A table file named like the program's executable (the source name without `.gloss`)
/// would be overwritten by the build, so it is refused on the table's line. A table whose
/// line already has an error (a bad or repeated file name) is left to that error.
fn executable_clashes(name: &str, text: &str, found: &[Diagnostic]) -> Vec<Diagnostic> {
    let source = Source::new(text);
    let Ok(program) = parse_source(&source) else {
        return Vec::new();
    };
    let executable = name.strip_suffix(".gloss").unwrap_or(name);
    let has_error = |line| found.iter().any(|d| d.is_error() && d.line == Some(line));
    let clashing = program
        .tables()
        .filter(|t| t.path == executable && !has_error(t.line));
    clashing
        .map(|t| {
            let msg = format!(
                "{} is the name of this program's executable; choose another file name",
                t.path
            );
            Diagnostic::error(&source, t.line, msg)
        })
        .collect()
}

fn format(path: &str, name: &str, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let formatted = match format_source(text) {
        Ok(f) => f,
        Err(d) => {
            let _ = writeln!(err, "{}", d.render());
            return 3;
        }
    };
    if formatted == text {
        let _ = writeln!(out, "{name} is already formatted");
    } else if let Err(e) = write_atomically(Path::new(path), formatted.as_bytes(), None) {
        let _ = writeln!(err, "cannot write {path}: {e}");
        return 3;
    } else {
        let _ = writeln!(out, "Formatted {name}");
    }
    0
}

/// Replaces the file at `path` with `text` by writing a short-named temporary file beside it
/// and renaming it over the original, so an interrupted write cannot truncate the user's
/// source. A symlink is followed (the real file is replaced, the link stays a link), a
/// read-only file is refused, and the original's permissions are kept. With a `mode`, the
/// file gets that mode instead, and need not exist yet.
pub(crate) fn write_atomically(
    path: &Path,
    bytes: &[u8],
    mode: Option<u32>,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let (real, permissions) = match mode {
        Some(mode) => (path.to_path_buf(), std::fs::Permissions::from_mode(mode)),
        None => {
            let real = std::fs::canonicalize(path)?;
            let permissions = std::fs::metadata(&real)?.permissions();
            (real, permissions)
        }
    };
    if mode.is_none() && permissions.readonly() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the file is read-only",
        ));
    }
    // O_EXCL, so a link planted at the temporary name is never followed.
    let (mut file, temp) = (0..100)
        .find_map(|n| {
            let name = format!(".gloss-write-{}-{n}.tmp", std::process::id());
            let temp = real.with_file_name(name);
            let opened = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp);
            match opened {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                other => Some(other.map(|f| (f, temp))),
            }
        })
        .unwrap_or_else(|| Err(std::io::Error::other("no free temporary file name")))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        std::fs::rename(&temp, &real)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In 3a nothing may ask a question, so any call fails the test.
    struct FakePrompt {
        terminal: bool,
    }

    impl Prompt for FakePrompt {
        fn is_terminal(&self) -> bool {
            self.terminal
        }
        fn ask(&mut self, question: &str) -> Option<String> {
            panic!("unexpected question: {question}");
        }
    }

    fn run_with(a: &[&str]) -> (i32, String) {
        let args: Vec<String> = a.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(
            &args,
            &mut out,
            &mut err,
            &mut FakePrompt { terminal: true },
        );
        (code, String::from_utf8(err).unwrap())
    }

    fn usage_error(a: &[&str]) {
        let (code, err) = run_with(a);
        assert_eq!((code, err.as_str()), (3, USAGE), "{a:?}");
    }

    #[test]
    fn allow_without_a_name_is_a_usage_error() {
        usage_error(&["build", "x.gloss", "--allow"]);
    }

    #[test]
    fn allow_needs_a_real_file_name() {
        usage_error(&["build", "x.gloss", "--allow", ""]);
        usage_error(&["build", "x.gloss", "--allow", "--allow", "x.csv"]);
        usage_error(&["build", "x.gloss", "--allow", "-q"]);
    }

    #[test]
    fn allow_has_one_spelling_and_follows_the_file() {
        usage_error(&["build", "x.gloss", "--allow=x.csv"]);
        usage_error(&["build", "--allow", "x.csv", "x.gloss"]);
    }

    #[test]
    fn allow_on_format_is_a_usage_error() {
        usage_error(&["format", "x.gloss", "--allow", "a.csv"]);
    }

    #[test]
    fn a_stray_word_is_a_usage_error() {
        usage_error(&["build", "x.gloss", "a.csv"]);
    }

    #[test]
    fn trace_accepts_allow() {
        let (code, err) = run_with(&["trace", "x.gloss", "--allow", "a.csv"]);
        assert_eq!(code, 3);
        assert!(err.contains("not built yet"), "{err}");
    }
}
