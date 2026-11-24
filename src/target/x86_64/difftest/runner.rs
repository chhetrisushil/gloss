//! Runs a module as a real executable. This mirrors `tests/run_support` (which a lib test
//! cannot use): the same runner lookup, concurrent pipes and deadline.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::Ran;
use crate::ir::Module;
use crate::target::x86_64_linux;

pub const DEADLINE: Duration = Duration::from_secs(10);

/// `GLOSS_RUNNER` (split on spaces), else `orb -m gloss-amd64` on macOS, else direct on
/// x86-64 Linux; `None` (after a visible skip) otherwise.
fn runner() -> Option<Vec<String>> {
    if let Ok(r) = std::env::var("GLOSS_RUNNER") {
        return Some(
            r.split(' ')
                .filter(|w| !w.is_empty())
                .map(String::from)
                .collect(),
        );
    }
    if cfg!(target_os = "macos") {
        return Some(["orb", "-m", "gloss-amd64"].map(String::from).to_vec());
    }
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some(Vec::new());
    }
    let _ = std::io::stderr().write_all(b"difftest skipped: no x86-64 Linux runner\n");
    None
}

/// Whether programs run through `orb`, the runner's first word exactly (as in `run_in`).
pub fn under_orb() -> bool {
    runner().is_some_and(|r| r.first().map(String::as_str) == Some("orb"))
}

/// A folder under the target directory, which the Linux machine can see (the system
/// temporary folder is not shared), removed on drop, also when a test panics.
/// `CARGO_TARGET_TMPDIR` is only set for integration tests, so it is built here.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        static RUNS: AtomicUsize = AtomicUsize::new(0);
        let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("target"),
            PathBuf::from,
        );
        let name = format!(
            "difftest-{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, Ordering::SeqCst)
        );
        let dir = target.join("tmp").join(name);
        std::fs::create_dir_all(&dir).expect("the temporary folder is created");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Builds and runs `m` with `stdin`; `None` without a runner. `Err` says why the program
/// did not exit: a timeout, or a signal.
pub fn run_program(m: &Module, stdin: &[u8], deadline: Duration) -> Option<Result<Ran, String>> {
    run_in(m, stdin, deadline, &TempDir::new())
}

/// Like `run_program`, with `dir` as the program's current folder (it also holds the
/// executable `prog` and the output files `prog.stdout` and `prog.stderr`).
pub fn run_in(
    m: &Module,
    stdin: &[u8],
    deadline: Duration,
    dir: &TempDir,
) -> Option<Result<Ran, String>> {
    let prefix = runner()?;
    let path = dir.0.join("prog");
    std::fs::write(&path, x86_64_linux().executable(m).expect("builds")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Under `orb` (the runner's first word, exactly; a wrapped runner such as `env orb ...` is
    // not covered) the program writes stdout and stderr to files in its shared folder, read
    // back once it has exited: `orb` drops the tail of piped output when the program exits
    // while output is in flight. Mirrors `tests/run_support`.
    let files = (prefix.first().map(String::as_str) == Some("orb"))
        .then(|| (path.with_extension("stdout"), path.with_extension("stderr")));
    let mut cmd = match prefix.split_first() {
        Some((program, args)) => {
            let mut c = Command::new(program);
            c.args(args);
            if let Some((o, e)) = &files {
                c.args(["sh", "-c", r#"exec "$0" >"$1" 2>"$2""#])
                    .args([&path, o, e]);
            } else {
                c.arg(&path);
            }
            c
        }
        None => Command::new(&path),
    };
    let mut child = cmd
        .current_dir(&dir.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the runner starts");
    let mut feed = child.stdin.take().expect("stdin");
    let input = stdin.to_vec();
    // A program may exit without reading its input: a broken pipe is not an error.
    let feeder = std::thread::spawn(move || drop(feed.write_all(&input)));
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    };
    let out = drain(Box::new(child.stdout.take().expect("stdout")));
    let err = drain(Box::new(child.stderr.take().expect("stderr")));
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("the program can be waited for") {
            break Some(s);
        }
        if start.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = feeder.join();
    let (mut out, mut err) = (out.join().expect("stdout"), err.join().expect("stderr"));
    if let (Some((o, e)), Some(_)) = (&files, status) {
        let read = |file: &Path| {
            std::fs::read(file).map_err(|why| {
                let runner_err = String::from_utf8_lossy(&err);
                format!(
                    "cannot read the program's output file {}: {why}; runner stderr: {runner_err}",
                    file.display()
                )
            })
        };
        match (read(o), read(e)) {
            (Ok(o), Ok(e)) => {
                out.extend(o);
                err.extend(e);
            }
            (Err(msg), _) | (_, Err(msg)) => return Some(Err(msg)),
        }
    }
    Some(match status.map(|s| s.code()) {
        None => Err(format!("timed out after {deadline:?}")),
        Some(None) => Err("ended by a signal".to_string()),
        Some(Some(code)) => Ok((code, out, err)),
    })
}
