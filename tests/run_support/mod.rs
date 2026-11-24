//! Finds a way to run x86-64 Linux executables, builds tiny ones and runs them.
#![allow(dead_code)] // shared by several test crates, each using some of it

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gloss::ir::Global;
use gloss::target::elf::{Image, write};
use gloss::target::x86_64::encode::encode;
use gloss::target::x86_64::inst::{Sym, X};

pub const SKIP: &str = "run tests skipped: no x86-64 Linux runner (set GLOSS_RUNNER)";
pub const DEADLINE: Duration = Duration::from_secs(10);

/// Exit status, standard output and standard error; or why the program did not exit.
pub type Ran = Result<(i32, Vec<u8>, Vec<u8>), String>;

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// The command prefix that runs an executable, empty to run it directly, `None` for no runner.
pub fn runner() -> Option<Vec<String>> {
    if let Some(v) = std::env::var_os("GLOSS_RUNNER") {
        let v = v.into_string().expect("GLOSS_RUNNER must be valid UTF-8");
        // Split on spaces by design (ruling M2-R11): a runner path with a space is unsupported.
        return Some(
            v.split(' ')
                .filter(|w| !w.is_empty())
                .map(String::from)
                .collect(),
        );
    }
    if cfg!(target_os = "macos") && on_path("orb") {
        return Some(["orb", "-m", "gloss-amd64"].map(String::from).to_vec());
    }
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some(Vec::new());
    }
    None
}

/// Prints the skip message straight to standard error, past the test harness's capture.
pub fn skip() {
    let _ = writeln!(std::io::stderr(), "{SKIP}");
}

pub const LIMITS_SKIP: &str =
    "memory-limit run skipped: the runner ignores ulimit -v for x86-64 code (QEMU)";

/// Whether the runner enforces `ulimit -v` on x86-64 programs: `dd` needs a 200 MB buffer,
/// which a 64 MB limit must refuse. QEMU user emulation ignores the limit, so a program
/// meant to run out of memory would instead be killed by the machine's OOM killer.
pub fn limits_hold(prefix: &[String]) -> bool {
    static HOLD: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HOLD.get_or_init(|| {
        let probe = "ulimit -v 65536; dd if=/dev/zero of=/dev/null bs=200M count=1";
        let mut words = prefix.iter().map(String::as_str).chain(["sh", "-c", probe]);
        let program = words.next().expect("the probe has a program");
        let status = Command::new(program)
            .args(words)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("the runner starts");
        !status.success()
    })
}

/// Runs `path` with `stdin`; `None` (after the skip message) when there is no runner.
pub fn run(path: &Path, stdin: &[u8]) -> Option<Ran> {
    run_within(path, stdin, DEADLINE)
}

pub fn run_within(path: &Path, stdin: &[u8], deadline: Duration) -> Option<Ran> {
    run_env(path, stdin, deadline, &[])
}

/// Like `run_within`, with extra environment variables (forwarded into the machine by
/// `ORBENV` under `orb`).
pub fn run_env(
    path: &Path,
    stdin: &[u8],
    deadline: Duration,
    vars: &[(&str, String)],
) -> Option<Ran> {
    run_limited(path, stdin, deadline, vars, None)
}

/// Like `run_env`, with `ulimit -v <kib>` applied in the shell that execs the program.
pub fn run_limited(
    path: &Path,
    stdin: &[u8],
    deadline: Duration,
    vars: &[(&str, String)],
    kib: Option<u64>,
) -> Option<Ran> {
    run_full(path, stdin, deadline, vars, kib, &[])
}

/// Like `run_limited`, with program arguments after the program's name.
pub fn run_full(
    path: &Path,
    stdin: &[u8],
    deadline: Duration,
    vars: &[(&str, String)],
    kib: Option<u64>,
    args: &[&str],
) -> Option<Ran> {
    run_core(path, stdin, deadline, vars, kib, args, None)
}

/// Like `run_full` with no variables, with `folder` as the program's current folder (set
/// with `current_dir` when run directly; under a runner the shell that runs the program
/// `cd`s there first) and `ulimit -v <kib>` applied when given.
pub fn run_case(
    path: &Path,
    stdin: &[u8],
    deadline: Duration,
    folder: &Path,
    args: &[&str],
    kib: Option<u64>,
) -> Option<Ran> {
    run_core(path, stdin, deadline, &[], kib, args, Some(folder))
}

fn run_core(
    path: &Path,
    stdin: &[u8],
    deadline: Duration,
    vars: &[(&str, String)],
    kib: Option<u64>,
    args: &[&str],
    folder: Option<&Path>,
) -> Option<Ran> {
    let Some(prefix) = runner() else {
        skip();
        return None;
    };
    if kib.is_some() && !limits_hold(&prefix) {
        let _ = writeln!(std::io::stderr(), "{LIMITS_SKIP}");
        return None;
    }
    // OrbStack's `orb` drops the tail of a program's stdout and stderr when the program exits
    // while output is still in flight, so under `orb` (the runner's first word, exactly; a
    // wrapped runner such as `env orb ...` is not covered) the program writes them to files in
    // its shared folder instead, read back once it has exited.
    let files = (prefix.first().map(String::as_str) == Some("orb")).then(|| {
        // Fresh names, so concurrent or repeated runs of one program never share a file.
        static RUNS: AtomicUsize = AtomicUsize::new(0);
        let n = RUNS.fetch_add(1, Ordering::SeqCst);
        let ext = |what: &str| path.with_extension(format!("{what}-{n}"));
        let files = (ext("stdout"), ext("stderr"));
        let _ = std::fs::remove_file(&files.0);
        let _ = std::fs::remove_file(&files.1);
        files
    });
    // The command line after the runner prefix. The paths, the limit, the folder and the
    // arguments are positional arguments of a fixed script, never interpolated into it:
    // the program, then the output files, the limit, the folder and the arguments.
    let limit = kib.map(|k| k.to_string());
    let mut tail: Vec<&std::ffi::OsStr> = vec![path.as_os_str()];
    let mut script = String::new();
    if let Some((o, e)) = &files {
        tail.extend([o.as_os_str(), e.as_os_str()]);
        script.push_str(r#"o="$1"; e="$2"; "#);
    }
    if let Some(l) = &limit {
        script.push_str(&format!(r#"ulimit -v "${}"; "#, tail.len()));
        tail.push(l.as_ref());
    }
    if let Some(f) = folder {
        script.push_str(&format!(r#"cd "${}" || exit 125; "#, tail.len()));
        tail.push(f.as_os_str());
    }
    let own = tail.len() - 1;
    tail.extend(args.iter().map(std::ffi::OsStr::new));
    // A limit, a folder or output files still need the `exec` that runs the program.
    if own > 0 || !args.is_empty() {
        let redirect = if files.is_some() {
            r#" >"$o" 2>"$e""#
        } else {
            ""
        };
        script.push_str(&format!(r#"shift {own}; exec "$0" "$@"{redirect}"#));
    }
    let script = (!script.is_empty()).then_some(script);
    if let Some(script) = &script {
        tail.splice(0..0, ["sh".as_ref(), "-c".as_ref(), script.as_ref()]);
    }
    let mut cmd = match prefix.split_first() {
        Some((program, args)) => {
            let mut c = Command::new(program);
            c.args(args).args(&tail);
            c
        }
        None if script.is_some() => {
            let mut c = Command::new(tail[0]);
            c.args(&tail[1..]);
            c
        }
        None => Command::new(path),
    };
    if let (true, Some(folder)) = (prefix.is_empty(), folder) {
        cmd.current_dir(folder);
    }
    cmd.envs(vars.iter().map(|(k, v)| (k, v)));
    let names: Vec<&str> = vars.iter().map(|(k, _)| *k).collect();
    if !names.is_empty() {
        cmd.env("ORBENV", names.join(":"));
    }
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
    let (mut out, mut err) = (out.join().expect("stdout"), err.join().expect("stderr"));
    if let (Some((o, e)), Some(_)) = (&files, status) {
        let read = read_back(o, e);
        let _ = (std::fs::remove_file(o), std::fs::remove_file(e));
        match read {
            Ok((o, e)) => {
                out.extend(o);
                err.extend(e);
            }
            Err(msg) => {
                let runner_err = String::from_utf8_lossy(&err);
                return Some(Err(format!("{msg}; runner stderr: {runner_err}")));
            }
        }
    }
    let stderr = String::from_utf8_lossy(&err);
    Some(match status {
        None => Err(format!("timed out after {deadline:?}; stderr: {stderr}")),
        Some(s) => match (s.code(), s.signal()) {
            (Some(code), _) => Ok((code, out, err)),
            (None, Some(sig)) => Err(format!("killed by signal {sig}; stderr: {stderr}")),
            (None, None) => Err(format!("ended without a status; stderr: {stderr}")),
        },
    })
}

/// The standard output and error a program redirected into files; an `Err` naming the file
/// that is missing or unreadable (never mistaken for empty output).
pub fn read_back(out: &Path, err: &Path) -> Result<(Vec<u8>, Vec<u8>), String> {
    let read = |p: &Path| {
        std::fs::read(p)
            .map_err(|e| format!("cannot read the program's output file {}: {e}", p.display()))
    };
    Ok((read(out)?, read(err)?))
}

/// A temporary folder under `target/`, which the Linux machine can see (the system temp
/// folder is not shared), removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = base.join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temporary folder is created");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Copies the files of `from` into `to`, creating folders as needed.
pub fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).unwrap();
        }
    }
}

/// Writes an executable (mode 0755) whose code is `xs`. `Sym::Text(0)` is the start of
/// `rodata` and `Global::OutBuf` the start of a data segment of `data_size` bytes.
pub fn executable(dir: &TempDir, xs: &[X], rodata: Vec<u8>, data_size: usize) -> PathBuf {
    let enc = encode(xs).expect("the code encodes");
    let img = Image {
        code: enc.bytes,
        entry: 0,
        fixups: enc.fixups,
        rodata,
        symbols: HashMap::from([(Sym::Text(0), 0)]),
        data_size,
        globals: HashMap::from([(Global::OutBuf, 0)]),
    };
    let path = dir.0.join("prog");
    std::fs::write(&path, write(&img).expect("the image is written")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}
