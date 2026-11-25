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
    /// The command's name as the refusal to approve says it: `Build` or `Trace`.
    pub command: &'static str,
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
        "build" | "format" | "test" | "trace" => 0,
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
    match command {
        "build" => {
            let options = BuildOptions {
                allow,
                command: "Build",
            };
            build(Path::new(file), &name, &text, &options, prompt, out, err)
        }
        "test" => test(&name, &text, out, err),
        "trace" => {
            let options = BuildOptions {
                allow,
                command: "Trace",
            };
            trace(Path::new(file), &name, &text, &options, prompt, out, err)
        }
        _ => format(file, &name, &text, out, err),
    }
}

/// `gloss test`: check, then build and run the program's tests from a private folder.
/// Nothing is written beside the source, and `.allow` is never read.
fn test(name: &str, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (mut diagnostics, checked) = crate::check_program(text);
    crate::diag::sort(&mut diagnostics);
    for d in &diagnostics {
        let _ = writeln!(err, "{}", d.render());
    }
    let Some(checked) = checked.filter(|_| !diagnostics.iter().any(|d| d.is_error())) else {
        return 3;
    };
    if checked.test_count() == 0 {
        let _ = writeln!(out, "No tests in {name}");
        return 0;
    }
    // After the errors above, so a Mac user still sees them.
    if !crate::testrun::host_runs_tests() {
        let _ = writeln!(err, "gloss test runs tests on Linux only");
        return 3;
    }
    match checked.test_executable() {
        Ok(bytes) => crate::testrun::run_test_executable(&bytes, out, err),
        Err(d) => {
            let _ = writeln!(err, "{}", d.render());
            3
        }
    }
}

/// The steps `build` and `trace` share: diagnostics, the `.gloss` clash rule, the `--allow`
/// flags and the file approval. `Err(status)` stops the command.
fn prepare(
    path: &Path,
    name: &str,
    text: &str,
    options: &BuildOptions,
    prompt: &mut dyn Prompt,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<crate::Checked, i32> {
    let (mut diagnostics, checked) = crate::check_program(text);
    let clashes = executable_clashes(name, text, &diagnostics);
    diagnostics.extend(clashes);
    crate::diag::sort(&mut diagnostics);
    for d in &diagnostics {
        let _ = writeln!(err, "{}", d.render());
    }
    if diagnostics.iter().any(|d| d.is_error()) {
        return Err(3);
    }
    let checked = checked.ok_or(3)?;
    let entries = checked.permissions();
    crate::approve::check_flags(&entries, &options.allow, err).map_err(|_| 3)?;
    if !entries.is_empty() {
        let _ = out.write_all(crate::permissions::render(&entries).as_bytes());
        crate::approve::approve(path, name, &entries, options, prompt, out, err)?;
    }
    Ok(checked)
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
    let checked = match prepare(path, name, text, options, prompt, out, err) {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    let Some(bytes) = rendered_or_none(checked.executable(), err) else {
        return 3;
    };
    let executable = path.with_extension("");
    if let Err(e) = write_atomically(&executable, &bytes, Some(0o755)) {
        let _ = writeln!(err, "cannot write {}: {e}", executable.display());
        return 3;
    }
    let _ = writeln!(out, "Built {}", name.strip_suffix(".gloss").unwrap_or(name));
    0
}

/// The bytes of a built executable, or `None` after printing why it could not be built.
fn rendered_or_none(built: Result<Vec<u8>, Diagnostic>, err: &mut dyn Write) -> Option<Vec<u8>> {
    built
        .inspect_err(|d| {
            let _ = writeln!(err, "{}", d.render());
        })
        .ok()
}

/// Lets the narrator and the runner both write to the one error stream.
struct Shared<'a, 'b>(&'a std::cell::RefCell<&'b mut dyn Write>);

impl Write for Shared<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

/// Whether `gloss trace` can run programs on this host (the same rule as `gloss test`).
fn host_traces(os: &str, arch: &str) -> bool {
    crate::testrun::host_can_run(os, arch)
}

/// What the event lines showed so far.
struct TraceState {
    /// The trace file's name, for its write errors.
    label: String,
    /// A `fail` event was seen.
    failed: bool,
    /// Why a line could not be read (`event N: <why>`); the run was stopped for it.
    unreadable: Option<String>,
}

/// Handles one event line: writes it and its newline to the trace file (a bad line too, so the
/// file is the program's output byte for byte), then narrates it. An error stops the run: a
/// write failure carries its message (`<name>.trace cannot be written: ...`), an unreadable
/// event is recorded in `state` and ends the run with a plain error.
fn handle_line(
    line: &str,
    file: &mut dyn Write,
    narrator: &mut crate::trace::narrate::Narrator,
    out: &mut dyn Write,
    err: &mut dyn Write,
    state: &mut TraceState,
) -> std::io::Result<()> {
    let written = file
        .write_all(line.as_bytes())
        .and_then(|()| file.write_all(b"\n"));
    written.map_err(|e| {
        std::io::Error::new(e.kind(), format!("{} cannot be written: {e}", state.label))
    })?;
    let narrated = narrator.line(line, out, err).map_err(|e| {
        std::io::Error::new(e.kind(), format!("cannot write the trace output: {e}"))
    })?;
    match narrated {
        Ok(crate::trace::narrate::Narrated::Failed) => state.failed = true,
        Ok(_) => {}
        Err(why) => {
            state.unreadable = Some(why);
            return Err(std::io::Error::other("unreadable event"));
        }
    }
    Ok(())
}

/// The status and the message for the end of a traced run. An unreadable event wins (3), then
/// a run error (3). A finished run is 0, or 2 when a `fail` event was seen; any other end is
/// 2 with the status in the message.
fn trace_outcome(
    ended: std::io::Result<crate::testrun::TraceEnd>,
    failed: bool,
    unreadable: Option<String>,
) -> (i32, Option<String>) {
    use crate::testrun::TraceEnd;
    if let Some(why) = unreadable {
        let message = format!("the traced run wrote an event gloss cannot read ({why})");
        return (3, Some(message));
    }
    match ended {
        Err(e) => (3, Some(e.to_string())),
        Ok(TraceEnd::Exited(0)) if !failed => (0, None),
        Ok(TraceEnd::Exited(0 | 2)) if failed => (2, None),
        Ok(TraceEnd::Exited(n) | TraceEnd::Unfinished(n)) => {
            let message = format!("the traced run ended without finishing (status {n})");
            (2, Some(message))
        }
    }
}

/// `gloss trace`: the steps of `build`, then run the traced executable, write each event line
/// to the `.trace` file beside the source and narrate it. 0 if the program ran to its end, 2 if
/// it failed or did not finish, 3 for a problem of gloss's own, 4 for no approval.
fn trace(
    path: &Path,
    name: &str,
    text: &str,
    options: &BuildOptions,
    prompt: &mut dyn Prompt,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let checked = match prepare(path, name, text, options, prompt, out, err) {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    // After the errors above, so a Mac user still sees them.
    if !host_traces(std::env::consts::OS, std::env::consts::ARCH) {
        let _ = writeln!(err, "gloss trace runs programs on Linux only");
        return 3;
    }
    let Some(bytes) = rendered_or_none(checked.trace_executable(), err) else {
        return 3;
    };
    let trace_path = path.with_extension("trace");
    let label = trace_path.file_name().map_or_else(
        || trace_path.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    let mut file = match std::fs::File::create(&trace_path) {
        Ok(file) => std::io::BufWriter::new(file),
        Err(e) => {
            let _ = writeln!(err, "{label} cannot be written: {e}");
            return 3;
        }
    };
    let mut state = TraceState {
        label,
        failed: false,
        unreadable: None,
    };
    // The narrator borrows `checked`'s program, so it lives only inside this block.
    let ended = {
        let mut narrator = crate::trace::narrate::Narrator::new(checked.program());
        let borrowed: &mut dyn Write = &mut *err;
        let shared = std::cell::RefCell::new(borrowed);
        let mut on_line = |line: &str| {
            let mut err = Shared(&shared);
            handle_line(line, &mut file, &mut narrator, out, &mut err, &mut state)
        };
        crate::testrun::run_traced(&bytes, &mut on_line, &mut Shared(&shared))
    };
    // The file is complete before any message is printed, on every way the run ended.
    let outcome = trace_outcome(ended, state.failed, state.unreadable);
    let (status, message) = flushed(&mut file, &state.label, outcome);
    if let Some(message) = message {
        let _ = writeln!(err, "{message}");
    }
    status
}

/// Flushes the trace file; a failure to do so is a status 3 with its reason, unless the run
/// already ended with a status 3 and its own message.
fn flushed(
    file: &mut dyn Write,
    label: &str,
    outcome: (i32, Option<String>),
) -> (i32, Option<String>) {
    match file.flush() {
        Ok(()) => outcome,
        Err(_) if outcome.0 == 3 => outcome,
        Err(e) => (3, Some(format!("{label} cannot be written: {e}"))),
    }
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

    /// A fresh folder for one test, removed afterwards.
    fn folder(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gloss-write-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_link_at_the_temporary_name_is_never_followed() {
        let dir = folder("link");
        let victim = dir.join("victim");
        std::fs::write(&victim, "keep").unwrap();
        let first = dir.join(format!(".gloss-write-{}-0.tmp", std::process::id()));
        std::os::unix::fs::symlink(&victim, &first).unwrap();
        write_atomically(&dir.join("t.allow"), b"new", Some(0o644)).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("t.allow")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
        assert!(std::fs::symlink_metadata(&first).unwrap().is_symlink());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writing_stops_when_every_temporary_name_is_taken() {
        let dir = folder("full");
        for n in 0..100 {
            let name = format!(".gloss-write-{}-{n}.tmp", std::process::id());
            std::fs::write(dir.join(name), "").unwrap();
        }
        let e = write_atomically(&dir.join("t.allow"), b"new", Some(0o644)).unwrap_err();
        assert_eq!(e.to_string(), "no free temporary file name");
        assert!(!dir.join("t.allow").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
        assert!(err.starts_with("cannot read x.gloss: "), "{err}");
    }

    #[test]
    fn trace_refuses_on_a_host_that_cannot_run_it() {
        assert!(host_traces("linux", "x86_64"));
        assert!(!host_traces("macos", "aarch64"));
        assert!(!host_traces("linux", "aarch64"));
    }

    /// A program with a show, to give the narrator something real.
    fn narrated(lines: &[&str], file: &mut dyn Write) -> (std::io::Result<()>, TraceState, String) {
        let (_, checked) = crate::check_program("feature start\n  show \"hi\"\n");
        let checked = checked.unwrap();
        let mut narrator = crate::trace::narrate::Narrator::new(checked.program());
        let mut state = TraceState {
            label: "t.trace".into(),
            failed: false,
            unreadable: None,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut result = Ok(());
        for line in lines {
            result = handle_line(line, file, &mut narrator, &mut out, &mut err, &mut state);
            if result.is_err() {
                break;
            }
        }
        let text = String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap();
        (result, state, text)
    }

    const SHOW: &str = r#"{"line":2,"event":"show","text":"hi"}"#;
    const FAIL: &str = r#"{"line":2,"event":"fail","failure":1,"message":"boom"}"#;

    #[test]
    fn every_line_reaches_the_file_with_its_newline() {
        let mut file = Vec::new();
        let (result, state, text) = narrated(&[SHOW, FAIL], &mut file);
        assert!(result.is_ok() && state.failed && state.unreadable.is_none());
        assert_eq!(
            String::from_utf8(file).unwrap(),
            format!("{SHOW}\n{FAIL}\n")
        );
        assert_eq!(text, "hi\nline 2: failure 1: boom\n");
    }

    #[test]
    fn an_unreadable_line_is_written_then_stops_the_run() {
        let mut file = Vec::new();
        let (result, state, _) = narrated(&[SHOW, "nonsense", SHOW], &mut file);
        assert!(result.is_err() && !state.failed);
        assert!(state.unreadable.unwrap().starts_with("event 2: "));
        assert_eq!(
            String::from_utf8(file).unwrap(),
            format!("{SHOW}\nnonsense\n")
        );
    }

    struct Full;
    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_failing_trace_file_stops_before_narrating() {
        let (result, _, text) = narrated(&[SHOW], &mut Full);
        assert_eq!(
            result.unwrap_err().to_string(),
            "t.trace cannot be written: disk full"
        );
        assert_eq!(text, "");
    }

    #[test]
    fn a_failing_narration_says_it_is_the_output() {
        let (_, checked) = crate::check_program("feature start\n  show \"hi\"\n");
        let checked = checked.unwrap();
        let mut narrator = crate::trace::narrate::Narrator::new(checked.program());
        let mut state = TraceState {
            label: "t.trace".into(),
            failed: false,
            unreadable: None,
        };
        let e = handle_line(
            SHOW,
            &mut Vec::new(),
            &mut narrator,
            &mut Full,
            &mut Vec::new(),
            &mut state,
        )
        .unwrap_err();
        assert_eq!(e.to_string(), "cannot write the trace output: disk full");
    }

    /// Counts what reaches it before and after a flush.
    #[derive(Default)]
    struct Staged {
        pending: Vec<u8>,
        on_disk: Vec<u8>,
        fail_flush: bool,
    }
    impl Write for Staged {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.pending.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.fail_flush {
                return Err(std::io::Error::other("disk full"));
            }
            let pending = std::mem::take(&mut self.pending);
            self.on_disk.extend(pending);
            Ok(())
        }
    }

    #[test]
    fn the_trace_file_is_flushed_whatever_the_status_and_a_failed_flush_is_3() {
        for outcome in [
            (0, None),
            (2, None),
            (
                2,
                Some("the traced run ended without finishing (status 1)".to_string()),
            ),
        ] {
            let mut file = Staged::default();
            file.write_all(b"x\n").unwrap();
            assert_eq!(flushed(&mut file, "t.trace", outcome.clone()), outcome);
            assert_eq!(file.on_disk, b"x\n");
        }
        let mut full = Staged {
            fail_flush: true,
            ..Staged::default()
        };
        let (status, message) = flushed(&mut full, "t.trace", (0, None));
        assert_eq!(
            (status, message.as_deref()),
            (3, Some("t.trace cannot be written: disk full"))
        );
        let own = (3, Some("gone".to_string()));
        assert_eq!(flushed(&mut full, "t.trace", own.clone()), own);
    }

    #[test]
    fn the_end_of_a_traced_run_gives_a_status_and_a_message() {
        use crate::testrun::TraceEnd::{Exited, Unfinished};
        let unfinished = |n| {
            Some(format!(
                "the traced run ended without finishing (status {n})"
            ))
        };
        assert_eq!(trace_outcome(Ok(Exited(0)), false, None), (0, None));
        assert_eq!(trace_outcome(Ok(Exited(0)), true, None), (2, None));
        assert_eq!(trace_outcome(Ok(Exited(2)), true, None), (2, None));
        assert_eq!(
            trace_outcome(Ok(Exited(2)), false, None),
            (2, unfinished(2))
        );
        assert_eq!(
            trace_outcome(Ok(Unfinished(139)), false, None),
            (2, unfinished(139))
        );
        assert_eq!(
            trace_outcome(Ok(Unfinished(0)), true, None),
            (2, unfinished(0))
        );
        let e = || std::io::Error::other("gone");
        assert_eq!(
            trace_outcome(Err(e()), false, None),
            (3, Some("gone".into()))
        );
        let why = Some("event 3: not an event".to_string());
        let read = Some(
            "the traced run wrote an event gloss cannot read (event 3: not an event)".to_string(),
        );
        assert_eq!(
            trace_outcome(Err(e()), false, why.clone()),
            (3, read.clone())
        );
        assert_eq!(trace_outcome(Ok(Unfinished(137)), true, why), (3, read));
    }
}
