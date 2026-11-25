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

/// Runs the script on the standard input this process was given, and prints what it saw.
#[test]
fn stdin_probe() {
    if std::env::var_os("GLOSS_STDIN_PROBE").is_none() {
        return;
    }
    let dir = PrivateDir::new().unwrap();
    let path = script(&dir, "if read line; then echo got; else echo none; fi");
    let mut out = Vec::new();
    run_program(&path, &mut out, &mut Vec::new());
    println!("probe:{}", String::from_utf8(out).unwrap().trim());
}

#[test]
fn standard_input_is_closed() {
    use std::process::Stdio;
    let mut me = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "testrun::tests::stdin_probe", "--nocapture"])
        .env("GLOSS_STDIN_PROBE", "1")
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
