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
    let prefix = runner()?;
    let dir = TempDir::new();
    let path = dir.0.join("prog");
    std::fs::write(&path, x86_64_linux().executable(m).expect("builds")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut cmd = match prefix.split_first() {
        Some((program, args)) => {
            let mut c = Command::new(program);
            c.args(args).arg(&path);
            c
        }
        None => Command::new(&path),
    };
    let mut child = cmd
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
    let (out, err) = (out.join().expect("stdout"), err.join().expect("stderr"));
    Some(match status.map(|s| s.code()) {
        None => Err(format!("timed out after {deadline:?}")),
        Some(None) => Err("ended by a signal".to_string()),
        Some(Some(code)) => Ok((code, out, err)),
    })
}
