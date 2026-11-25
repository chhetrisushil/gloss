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
    let dir = match PrivateDir::new() {
        Ok(dir) => dir,
        Err(e) => {
            let _ = writeln!(err, "cannot make a private folder for the test run: {e}");
            return 3;
        }
    };
    let path = dir.path().join("tests");
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)
        .and_then(|mut f| f.write_all(executable));
    if let Err(e) = written {
        let _ = writeln!(err, "cannot write the test executable: {e}");
        return 3;
    }
    run_program(&path, out, err)
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
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let _ = writeln!(err, "cannot run the test executable: {e}");
            return 3;
        }
    };
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
        let sink: &mut dyn Write = if is_err { &mut *err } else { &mut *out };
        if let Err(e) = sink.write_all(&chunk).and_then(|()| sink.flush()) {
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
        let _ = writeln!(err, "cannot write the test output: {e}");
        return 3;
    }
    for r in readers {
        let _ = r.join();
    }
    let (status, message) = match child.wait() {
        Ok(ended) => status_for(ended),
        Err(e) => (3, Some(format!("cannot wait for the test run: {e}"))),
    };
    if let Some(message) = message {
        let _ = writeln!(err, "{message}");
    }
    status
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

/// 0 and 1 are the test run's own; any other end is 1 with a message.
pub fn status_for(ended: ExitStatus) -> (i32, Option<String>) {
    let code = match (ended.code(), ended.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => 256 + signal,
        (None, None) => 256,
    };
    match code {
        0 | 1 => (code, None),
        n => (
            1,
            Some(format!("the test run ended without finishing (status {n})")),
        ),
    }
}

#[cfg(test)]
mod tests;
