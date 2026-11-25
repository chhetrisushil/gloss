use super::*;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::mpsc;

#[test]
fn the_private_folder_is_0700_and_removed() {
    let a = PrivateDir::new().unwrap();
    let b = PrivateDir::new().unwrap();
    let mode = std::fs::metadata(a.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_ne!(a.path(), b.path());
    let path = a.path().to_path_buf();
    drop(a);
    assert!(!path.exists());
}

#[test]
fn names_carry_the_process_id_and_do_not_repeat() {
    let (a, b) = (candidate_name(), candidate_name());
    assert_ne!(a, b);
    assert!(
        a.starts_with(&format!("gloss-test-{}-", std::process::id())),
        "{a}"
    );
    // The random part differs even when everything else could be guessed.
    let random = |n: &str| n.rsplit('-').next().unwrap().to_string();
    assert_eq!(random(&a).len(), 16);
    assert_ne!(random(&a), random(&b));
}

#[test]
fn a_taken_name_is_skipped() {
    let base = PrivateDir::new().unwrap();
    std::fs::create_dir(base.path().join("first")).unwrap();
    let mut names = ["first", "second"].into_iter().map(String::from);
    let dir = PrivateDir::create_in(base.path(), &mut names).unwrap();
    assert_eq!(dir.path(), base.path().join("second"));
    assert!(base.path().join("first").exists());
    // With every name taken it fails instead of looping.
    let mut taken = ["first"].into_iter().map(String::from);
    assert!(PrivateDir::create_in(base.path(), &mut taken).is_err());
}

#[test]
fn a_panic_still_removes_the_folder() {
    let mut path = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let dir = PrivateDir::new().unwrap();
        path = Some(dir.path().to_path_buf());
        panic!("boom");
    }));
    assert!(result.is_err());
    assert!(!path.unwrap().exists());
}

#[test]
fn status_for_maps_exit_codes() {
    let at = |raw| status_for(ExitStatus::from_raw(raw));
    assert_eq!(at(0), (0, None));
    assert_eq!(at(1 << 8), (1, None));
    let ended = |n: i32| Some(format!("the test run ended without finishing (status {n})"));
    assert_eq!(at(7 << 8), (1, ended(7)));
    assert_eq!(at(11), (1, ended(267)));
}

#[test]
fn host_decision_is_linux_x86_64_only() {
    assert!(host_can_run("linux", "x86_64"));
    assert!(!host_can_run("linux", "aarch64"));
    assert!(!host_can_run("macos", "aarch64"));
    assert!(!host_can_run("macos", "x86_64"));
}

fn script(dir: &PrivateDir, body: &str) -> PathBuf {
    let path = dir.path().join("prog");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn a_script_stands_in_for_the_executable() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "echo x; echo y >&2; exit 1");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_program(&path, &mut out, &mut err), 1);
    assert_eq!(out, b"x\n");
    assert_eq!(err, b"y\n");
}

/// Runs the script on the standard input this process was given, and prints what it saw. The
/// run gives the script a null input whatever this process has, so it must see none; the
/// test below runs it again with an input that has bytes waiting.
#[test]
fn stdin_probe() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "if read line; then echo got; else echo none; fi");
    let mut out = Vec::new();
    run_program(&path, &mut out, &mut Vec::new());
    let seen = String::from_utf8(out).unwrap();
    assert_eq!(seen.trim(), "none");
    println!("probe:{}", seen.trim());
}

#[test]
fn a_relative_base_still_gives_an_absolute_folder_the_program_runs_from() {
    let mut names = std::iter::once(candidate_name());
    let dir = PrivateDir::create_in(Path::new("."), &mut names).unwrap();
    assert!(dir.path().is_absolute(), "{:?}", dir.path());
    let path = script(&dir, "echo ran");
    let mut out = Vec::new();
    assert_eq!(run_program(&path, &mut out, &mut Vec::new()), 0);
    assert_eq!(out, b"ran\n");
}

#[test]
fn the_program_runs_in_the_private_folder() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "pwd -P");
    let mut out = Vec::new();
    assert_eq!(run_program(&path, &mut out, &mut Vec::new()), 0);
    let want = dir.path().canonicalize().unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap().trim(),
        want.to_str().unwrap()
    );
}

#[test]
fn standard_input_is_closed() {
    use std::process::Stdio;
    let mut me = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "testrun::tests::stdin_probe", "--nocapture"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    me.stdin.take().unwrap().write_all(b"hello\n").unwrap();
    let done = me.wait_with_output().unwrap();
    let text = String::from_utf8(done.stdout).unwrap();
    assert!(text.contains("probe:none"), "{text}");
}

#[test]
fn a_signal_ends_the_run_with_a_message() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "kill -9 $$");
    let mut err = Vec::new();
    assert_eq!(run_program(&path, &mut Vec::new(), &mut err), 1);
    let text = String::from_utf8(err).unwrap();
    assert_eq!(text, "the test run ended without finishing (status 265)\n");
}

/// Holds what is written until it is flushed, as a buffered terminal would.
struct Chan(mpsc::Sender<Vec<u8>>, Vec<u8>);
impl Write for Chan {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let _ = self.0.send(std::mem::take(&mut self.1));
        Ok(())
    }
}

#[test]
fn output_is_streamed_before_the_run_ends() {
    let dir = PrivateDir::new().unwrap();
    let fifo = dir.path().join("gate");
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let path = script(
        &dir,
        &format!("echo x; read go < '{}'; echo z", fifo.display()),
    );
    let (tx, rx) = mpsc::channel();
    let runner =
        std::thread::spawn(move || run_program(&path, &mut Chan(tx, Vec::new()), &mut Vec::new()));
    // The script is still blocked on the FIFO, so only streaming can deliver `x`.
    let wait = std::time::Duration::from_secs(10);
    assert_eq!(rx.recv_timeout(wait).unwrap(), b"x\n");
    std::fs::write(&fifo, "go\n").unwrap();
    assert_eq!(runner.join().unwrap(), 0);
    assert_eq!(rx.recv_timeout(wait).unwrap(), b"z\n");
}

#[test]
fn a_busy_executable_is_retried() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "echo x");
    let mut calls = 0;
    let mut start = |p: &Path| {
        calls += 1;
        if calls == 1 {
            return Err(std::io::Error::from_raw_os_error(ETXTBSY));
        }
        spawn(p)
    };
    let mut out = Vec::new();
    assert_eq!(
        run_program_with(&path, &mut out, &mut Vec::new(), &mut start),
        0
    );
    assert_eq!((calls, out.as_slice()), (2, b"x\n".as_slice()));
}

#[test]
fn another_spawn_error_is_not_retried() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "echo x");
    let mut calls = 0;
    let mut start = |_: &Path| {
        calls += 1;
        Err(std::io::Error::from_raw_os_error(2))
    };
    let mut err = Vec::new();
    assert_eq!(
        run_program_with(&path, &mut Vec::new(), &mut err, &mut start),
        3
    );
    assert_eq!(calls, 1);
    assert!(
        String::from_utf8(err)
            .unwrap()
            .starts_with("cannot run the test executable: ")
    );
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("gone"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failing_writer_ends_the_run_with_status_3() {
    let dir = PrivateDir::new().unwrap();
    // `exec` makes the sleep the child itself, so killing the child ends it.
    let path = script(&dir, "echo x; exec sleep 30");
    let started = std::time::Instant::now();
    let mut err = Vec::new();
    assert_eq!(run_program(&path, &mut Broken, &mut err), 3);
    assert!(started.elapsed().as_secs() < 20, "the child was not killed");
    let text = String::from_utf8(err).unwrap();
    assert_eq!(text, "cannot write the test output: gone\n");
}

/// Runs a script as a traced program on this process's standard input; gives the lines, the
/// standard error text and how it ended.
fn traced(body: &str) -> (Vec<String>, String, TraceEnd) {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, body);
    let (mut lines, mut err) = (Vec::new(), Vec::new());
    let mut on_line = |l: &str| {
        lines.push(l.to_string());
        Ok(())
    };
    let end = run_traced_with(&path, &mut on_line, &mut err, &mut spawn_inheriting).unwrap();
    (lines, String::from_utf8(err).unwrap(), end)
}

#[test]
fn two_events_and_exit_0() {
    let (lines, err, end) = traced("echo '{\"a\":1}'; echo '{\"b\":2}'; echo oops >&2");
    assert_eq!(lines, ["{\"a\":1}", "{\"b\":2}"]);
    assert_eq!(err, "oops\n");
    assert_eq!(end, TraceEnd::Exited(0));
}

#[test]
fn a_line_split_across_two_writes_arrives_whole() {
    let (lines, _, end) =
        traced("printf '{\"a\":'; sleep 0.2; printf '1}\\n{\"b\"'; sleep 0.2; echo ':2}'");
    assert_eq!(lines, ["{\"a\":1}", "{\"b\":2}"]);
    assert_eq!(end, TraceEnd::Exited(0));
}

#[test]
fn status_2_is_exited_and_others_are_unfinished() {
    assert_eq!(traced("echo x; exit 2").2, TraceEnd::Exited(2));
    assert_eq!(traced("echo x; exit 1").2, TraceEnd::Unfinished(1));
}

#[test]
fn a_last_fragment_without_a_newline_is_unfinished() {
    let (lines, _, end) = traced("echo x; printf '{\"a\"'");
    assert_eq!(lines, ["x"]);
    assert_eq!(end, TraceEnd::Unfinished(0));
}

#[test]
fn a_signal_is_unfinished_with_256_plus_the_signal() {
    assert_eq!(traced("kill -9 $$").2, TraceEnd::Unfinished(265));
}

#[test]
fn a_failing_line_handler_ends_the_run() {
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "echo x; exec sleep 5");
    let mut on_line = |_: &str| Err(std::io::Error::other("no room"));
    let started = std::time::Instant::now();
    let e =
        run_traced_with(&path, &mut on_line, &mut Vec::new(), &mut spawn_inheriting).unwrap_err();
    assert!(e.to_string().contains("no room"), "{e}");
    assert!(started.elapsed().as_secs() < 4);
}

#[test]
fn standard_input_reaches_the_program_and_it_runs_in_the_current_folder() {
    use std::process::Stdio;
    let mut me = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "testrun::tests::stdin_reaches_probe",
            "--ignored",
            "--nocapture",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    me.stdin.take().unwrap().write_all(b"hello\n").unwrap();
    let done = me.wait_with_output().unwrap();
    let text = String::from_utf8(done.stdout).unwrap();
    let here = std::env::current_dir().unwrap().canonicalize().unwrap();
    let want = format!("probe:got hello in {}", here.display());
    assert!(text.contains(&want), "{text}");
}

/// Run by the test above with a standard input that has bytes waiting.
#[test]
#[ignore = "run by the test above with an input; alone it would wait for this process's input"]
fn stdin_reaches_probe() {
    let (lines, _, _) = traced("read l; echo \"got $l in $(pwd -P)\"");
    println!("probe:{}", lines.join(""));
}

#[test]
fn run_traced_runs_a_written_executable() {
    let mut lines = Vec::new();
    let mut on_line = |l: &str| {
        lines.push(l.to_string());
        Ok(())
    };
    let end = run_traced(
        b"#!/bin/sh\necho '{\"k\":1}'\n",
        &mut on_line,
        &mut Vec::new(),
    );
    assert_eq!(end.unwrap(), TraceEnd::Exited(0));
    assert_eq!(lines, ["{\"k\":1}"]);
}
