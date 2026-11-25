//! The test executable `gloss test` builds runs on Linux and reports each test.

mod run_support;

use gloss::ir::Reg::{A, B};
use gloss::ir::{Cond, Function, Inst, Label, Module, Prim};
use gloss::target::x86_64::inst::{AluOp, Mem, R, X};
use gloss::target::x86_64::linux::{
    EXECVE, EXIT_GROUP, RT_SIGACTION, SIG_IGN, SIGACTION_SIZE, SIGCHLD, SIGSET_SIZE,
};
use gloss::target::x86_64_linux;
use run_support::{TempDir, executable, run, run_full};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

const PASSING: &str = "feature start\n  show 1\n\ntest \"t\"\n  set y to 1\n";

fn built(dir: &TempDir, text: &str) -> PathBuf {
    let bytes = gloss::build_test_executable(text).expect("builds");
    let path = dir.0.join("tests");
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const REPORT: &[u8] = b"pass  t\n\n1 passed, 0 failed\n";

#[test]
fn a_test_executable_reports_a_pass() {
    let dir = TempDir::new("test-runs-pass");
    if let Some(ran) = run(&built(&dir, PASSING), b"") {
        assert_eq!(ran.unwrap(), (0, REPORT.to_vec(), Vec::new()));
    }
}

/// An image that ignores `SIGCHLD` (`rt_sigaction`) and then runs its first argument, as a
/// parent that ignores it would start the program: the disposition survives `execve`. A shell
/// cannot do it here (busybox's `trap` leaves `SIGCHLD` alone).
fn launcher(dir: &TempDir) -> PathBuf {
    // The kernel's `struct sigaction` is the handler, the flags, the restorer and the mask.
    let (flags, restorer, mask) = (0, 0, 0);
    let words = [mask, restorer, flags, SIG_IGN];
    let mut ignore: Vec<X> = Vec::new();
    for word in words {
        ignore.extend([X::MovRI(R::R11, word), X::Push(R::R11)]);
    }
    ignore.extend([
        X::MovRI(R::Rdi, SIGCHLD),
        X::MovRR(R::Rsi, R::Rsp),
        X::MovRI(R::Rdx, 0),
        X::MovRI(R::R10, SIGSET_SIZE),
        X::MovRI(R::Rax, RT_SIGACTION),
        X::Syscall,
        X::AluI(AluOp::Add, R::Rsp, SIGACTION_SIZE),
    ]);
    // `execve(argv[1], &argv[1], NULL)` (at rsp: argc, argv[0], argv[1]...), and exit
    // `EXEC_FAILED` if it fails.
    let argv1 = Mem::Base(R::Rsp, 16);
    let exec = [
        X::MovRM(R::Rdi, argv1.clone()),
        X::Lea(R::Rsi, argv1),
        X::MovRI(R::Rdx, 0),
        X::MovRI(R::Rax, EXECVE),
        X::Syscall,
        X::MovRI(R::Rdi, EXEC_FAILED),
        X::MovRI(R::Rax, EXIT_GROUP),
        X::Syscall,
    ];
    executable(dir, &[&ignore[..], &exec].concat(), Vec::new(), 0)
}

/// The launcher's exit status when it cannot run its argument.
const EXEC_FAILED: i64 = 127;

/// Runs `exe` through the launcher.
fn run_ignoring_sigchld(parent: &TempDir, exe: &std::path::Path) -> Option<run_support::Ran> {
    let args = [exe.to_str().unwrap()];
    run_full(
        &launcher(parent),
        b"",
        Duration::from_secs(60),
        &[],
        None,
        &args,
    )
}

#[test]
fn a_test_executable_started_with_sigchld_ignored_still_reports_a_pass() {
    // With SIGCHLD ignored, the kernel reaps children itself and every Wait would fail with
    // no such child, reporting the test lost; the executable resets the signal at its start.
    let (tests, parent) = (
        TempDir::new("test-runs-sigchld"),
        TempDir::new("test-runs-parent"),
    );
    if let Some(ran) = run_ignoring_sigchld(&parent, &built(&tests, PASSING)) {
        assert_eq!(ran.unwrap(), (0, REPORT.to_vec(), Vec::new()));
    }
}

/// A module that forks a child exiting 0 and exits with `Wait`'s result for it, the first
/// thing it does being `DefaultChildSignal` when `reset` is set.
fn fork_and_wait(reset: bool) -> Module {
    let mut body = vec![Inst::Enter(1)];
    if reset {
        body.push(Inst::System(Prim::DefaultChildSignal));
    }
    body.extend([
        Inst::System(Prim::Fork),
        Inst::StoreSlot(1, A),
        Inst::Const(B, 0),
        Inst::Compare,
        Inst::JumpIf(Cond::Eq, Label(1)),
        Inst::LoadSlot(A, 1),
        Inst::Push(A),
        Inst::System(Prim::Wait),
        Inst::Push(A),
        Inst::System(Prim::Exit),
        Inst::Mark(Label(1)),
        Inst::Const(A, 0),
        Inst::Push(A),
        Inst::System(Prim::Exit),
    ]);
    let mut m = Module::default();
    m.functions.push(Function {
        name: "_start".into(),
        params: 0,
        slots: 1,
        body,
    });
    m
}

#[test]
fn the_launcher_really_ignores_sigchld_and_the_reset_undoes_it() {
    // The control for the test above: a Wait through the same launcher gives -10 (the low
    // byte of it is the status) unless the program resets the signal first.
    let status_of = |reset: bool, name: &str| {
        let (dir, parent) = (TempDir::new(name), TempDir::new("test-runs-control"));
        let bytes = x86_64_linux().executable(&fork_and_wait(reset)).unwrap();
        let path = dir.0.join("prog");
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        run_ignoring_sigchld(&parent, &path).map(|ran| ran.unwrap().0)
    };
    if let (Some(plain), Some(reset)) = (
        status_of(false, "control-plain"),
        status_of(true, "control-reset"),
    ) {
        assert_eq!((plain, reset), ((-10i64 & 0xff) as i32, 0));
    }
}
