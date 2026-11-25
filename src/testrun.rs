//! Running a test executable on the host: a private folder, streamed output, one exit status.
//! This is host code, not generated code, so it may decode an `ExitStatus`.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

/// Whether this host can run the executables the compiler builds.
pub fn host_runs_tests() -> bool {
    host_can_run(std::env::consts::OS, std::env::consts::ARCH)
}

/// The decision behind `host_runs_tests`, for any operating system and processor.
fn host_can_run(os: &str, arch: &str) -> bool {
    os == "linux" && arch == "x86_64"
}

/// A fresh private folder (mode 0700) under the system temporary folder, removed on drop.
pub struct PrivateDir {
    path: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl PrivateDir {
    /// Creates the folder under a name no other folder has.
    pub fn new() -> std::io::Result<PrivateDir> {
        Self::create_in(
            &std::env::temp_dir(),
            &mut std::iter::from_fn(|| Some(candidate_name())),
        )
    }

    /// Creates the folder in `base` under the first of `names` that is free.
    fn create_in(
        base: &Path,
        names: &mut dyn Iterator<Item = String>,
    ) -> std::io::Result<PrivateDir> {
        // Always absolute: the program runs with this folder as its working directory, so a
        // relative path to it (a relative TMPDIR) would be resolved again from inside it.
        let base = std::path::absolute(base)?;
        for name in names {
            let path = base.join(name);
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(PrivateDir { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("no folder name was free"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A folder name that another process cannot predict: the process id, the time in
/// nanoseconds, a counter and a random number (std's randomly keyed hasher).
fn candidate_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    format!(
        "gloss-test-{}-{nanos}-{n}-{random:016x}",
        std::process::id()
    )
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Writes `executable` into a private folder (mode 0700), runs it with standard input closed,
/// streams its output as it arrives, and gives the exit status for `gloss test`.
pub fn run_test_executable(executable: &[u8], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (dir, path) = match write_executable(executable, "tests") {
        Ok(made) => made,
        Err((what, e)) => {
            let _ = writeln!(err, "{what}: {e}");
            return 3;
        }
    };
    let status = run_program(&path, out, err);
    drop(dir);
    status
}

/// Puts `executable` under `name` in a new private folder; on failure says which step failed.
fn write_executable(
    executable: &[u8],
    name: &str,
) -> Result<(PrivateDir, PathBuf), (&'static str, std::io::Error)> {
    let dir =
        PrivateDir::new().map_err(|e| ("cannot make a private folder for the test run", e))?;
    let path = dir.path().join(name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)
        .and_then(|mut f| f.write_all(executable))
        .map_err(|e| ("cannot write the test executable", e))?;
    Ok((dir, path))
}

/// Runs the program at `path`, copying its output to `out` and `err` as bytes arrive.
fn run_program(path: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_program_with(path, out, err, &mut spawn)
}

/// Starts the program with standard input null and standard output and error piped, and the
/// private folder it sits in as its working directory.
fn spawn(path: &Path) -> std::io::Result<Child> {
    let mut command = Command::new(path);
    if let Some(folder) = path.parent().filter(|f| !f.as_os_str().is_empty()) {
        command.current_dir(folder);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// `ETXTBSY`: the file is still open for writing somewhere. Another thread that forks while
/// we write the executable can hold the write end for a moment, so the spawn is retried.
const ETXTBSY: i32 = 26;

/// `run_program` with the way the child is started given, so a test can make a spawn fail.
fn run_program_with(
    path: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
    start: &mut dyn FnMut(&Path) -> std::io::Result<Child>,
) -> i32 {
    let mut on_out = |chunk: &[u8]| out.write_all(chunk).and_then(|()| out.flush());
    match drive(path, &mut on_out, err, start) {
        Ok(ended) => {
            let (status, message) = status_for(ended);
            if let Some(message) = message {
                let _ = writeln!(err, "{message}");
            }
            status
        }
        Err(failure) => {
            let _ = writeln!(err, "{}", failure.message());
            3
        }
    }
}

/// Why a run could not be driven to the end.
enum Failure {
    Start(std::io::Error),
    /// Writing standard error failed.
    Output(std::io::Error),
    /// The consumer of standard output (`on_out`) failed.
    Sink(std::io::Error),
    Wait(std::io::Error),
}

impl Failure {
    fn source(&self) -> &std::io::Error {
        match self {
            Failure::Start(e) | Failure::Output(e) | Failure::Sink(e) | Failure::Wait(e) => e,
        }
    }

    /// The error a caller of `run_traced` gets: the consumer's own error unchanged, any other
    /// failure with its message.
    fn into_error(self) -> std::io::Error {
        if let Failure::Sink(e) = self {
            return e;
        }
        std::io::Error::new(self.source().kind(), self.message())
    }

    fn message(&self) -> String {
        match self {
            Failure::Start(e) => format!("cannot run the test executable: {e}"),
            Failure::Output(e) | Failure::Sink(e) => {
                format!("cannot write the test output: {e}")
            }
            Failure::Wait(e) => format!("cannot wait for the test run: {e}"),
        }
    }
}

/// Starts the program (retrying a busy file), gives each chunk of its standard output to
/// `on_out` as it arrives, copies its standard error to `err`, and returns how it ended.
fn drive(
    path: &Path,
    on_out: &mut dyn FnMut(&[u8]) -> std::io::Result<()>,
    err: &mut dyn Write,
    start: &mut dyn FnMut(&Path) -> std::io::Result<Child>,
) -> Result<ExitStatus, Failure> {
    let mut tries = 0;
    let spawned = loop {
        match start(path) {
            Err(e) if e.raw_os_error() == Some(ETXTBSY) && tries < 5 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10 << tries));
            }
            other => break other,
        }
    };
    // Ends the child if this function is left early, by a panic in `on_out` for one.
    let mut child = Reaper(spawned.map_err(Failure::Start)?);
    let (tx, rx) = mpsc::channel::<(bool, Vec<u8>)>();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(pump(stdout, false, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(pump(stderr, true, tx.clone()));
    }
    drop(tx);
    let mut failed = None;
    for (is_err, chunk) in &rx {
        let written = if is_err {
            err.write_all(&chunk)
                .and_then(|()| err.flush())
                .map_err(Failure::Output)
        } else {
            on_out(&chunk).map_err(Failure::Sink)
        };
        if let Err(e) = written {
            failed = Some(e);
            break;
        }
    }
    if let Some(e) = failed {
        // Nobody is reading the output any more: end the run and reap the child.
        let _ = child.kill();
        drop(rx);
        let _ = child.wait();
        for r in readers {
            let _ = r.join();
        }
        return Err(e);
    }
    for r in readers {
        let _ = r.join();
    }
    child.wait().map_err(Failure::Wait)
}

/// A started child that is killed and reaped when dropped; a no-op once it has been waited for.
struct Reaper(Child);

impl std::ops::Deref for Reaper {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for Reaper {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Reaper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Reads `from` on its own thread, sending each chunk as it arrives.
fn pump(
    mut from: impl Read + Send + 'static,
    is_err: bool,
    to: mpsc::Sender<(bool, Vec<u8>)>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 || to.send((is_err, buf[..n].to_vec())).is_err() {
                break;
            }
        }
    })
}

/// The exit code, or 256 plus the signal that ended the program.
fn exit_code(ended: ExitStatus) -> i32 {
    match (ended.code(), ended.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => 256 + signal,
        (None, None) => 256,
    }
}

/// 0 and 1 are the test run's own; any other end is 1 with a message.
pub fn status_for(ended: ExitStatus) -> (i32, Option<String>) {
    let code = exit_code(ended);
    match code {
        0 | 1 => (code, None),
        n => (
            1,
            Some(format!("the test run ended without finishing (status {n})")),
        ),
    }
}

/// How a traced run ended: it finished (status 0, or 2 for a run-time failure the program
/// reported itself), or it did not (any other status, or a last line cut short).
#[derive(Debug, PartialEq, Eq)]
pub enum TraceEnd {
    Exited(i32),
    Unfinished(i32),
}

/// Starts the program with standard input and working folder inherited from this process, and
/// standard output and error piped.
fn spawn_inheriting(path: &Path) -> std::io::Result<Child> {
    Command::new(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Splits the bytes of standard output at `\n` into lines, whatever the chunks are.
#[derive(Default)]
struct LineSplitter {
    /// The start of a line whose newline has not arrived yet.
    rest: Vec<u8>,
    /// How many lines have been handed on.
    lines: usize,
}

impl LineSplitter {
    /// Hands each line the chunk completes to `on_line`, without its newline. Only the new
    /// bytes are scanned. A line that is not UTF-8 is an `InvalidData` error naming its number:
    /// events are always text, so such bytes mean the stream is corrupt.
    fn feed(
        &mut self,
        chunk: &[u8],
        on_line: &mut dyn FnMut(&str) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut start = 0;
        while let Some(found) = chunk[start..].iter().position(|&b| b == b'\n') {
            let end = start + found;
            self.rest.extend_from_slice(&chunk[start..end]);
            start = end + 1;
            self.lines += 1;
            let line = std::mem::take(&mut self.rest);
            let text = std::str::from_utf8(&line).map_err(|_| {
                let n = self.lines;
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {n} of the program's output is not text"),
                )
            })?;
            on_line(text)?;
        }
        self.rest.extend_from_slice(&chunk[start..]);
        Ok(())
    }

    /// Whether a last line is still waiting for its newline.
    fn unfinished(&self) -> bool {
        !self.rest.is_empty()
    }
}

/// Runs a traced executable: standard output is one JSON event per line, each handed whole to
/// `on_line` (no limit on a line's length); standard error goes to `err`. A failing `on_line`
/// ends the run and its error is returned unchanged; a failing `err` ends it too. Output that
/// is not UTF-8 is an `InvalidData` error.
///
/// `err` and `on_line` are both called from this thread, in the order their bytes arrive, so
/// a narrator line is never split inside another write; but standard output is held until a
/// newline while standard error is passed at once, so the program's stderr and the narration
/// interleave by arrival. A child that never closes its standard output keeps the run going
/// until it ends or is interrupted (accepted, like D49). When the run ends early, the child is
/// killed and the rest of its standard error is dropped.
pub fn run_traced(
    executable: &[u8],
    on_line: &mut dyn FnMut(&str) -> std::io::Result<()>,
    err: &mut dyn Write,
) -> std::io::Result<TraceEnd> {
    let (dir, path) = write_executable(executable, "traced")
        .map_err(|(what, e)| std::io::Error::new(e.kind(), format!("{what}: {e}")))?;
    let end = run_traced_with(&path, on_line, err, &mut spawn_inheriting);
    drop(dir);
    end
}

/// `run_traced` for a program already in place, with the way it is started given.
fn run_traced_with(
    path: &Path,
    on_line: &mut dyn FnMut(&str) -> std::io::Result<()>,
    err: &mut dyn Write,
    start: &mut dyn FnMut(&Path) -> std::io::Result<Child>,
) -> std::io::Result<TraceEnd> {
    let mut splitter = LineSplitter::default();
    let mut on_out = |chunk: &[u8]| splitter.feed(chunk, on_line);
    let ended = drive(path, &mut on_out, err, start).map_err(Failure::into_error)?;
    let code = exit_code(ended);
    Ok(match code {
        0 | 2 if !splitter.unfinished() => TraceEnd::Exited(code),
        n => TraceEnd::Unfinished(n),
    })
}

#[cfg(test)]
mod tests;
