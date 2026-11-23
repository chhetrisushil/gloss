//! The runner copes with programs that ignore stdin, flood stdout, hang or are killed.

mod run_support;

use std::time::Duration;

use gloss::ir::Global;
use gloss::target::x86_64::inst::{AluOp, Mem, R, Sym, X};
use run_support::{TempDir, executable, read_back, run, run_within, runner};

const MIB: usize = 1 << 20;

fn syscall(n: i64, rdi: i64) -> Vec<X> {
    vec![
        X::MovRI(R::Rax, n),
        X::MovRI(R::Rdi, rdi),
        X::MovRI(R::Rsi, 0),
        X::Syscall,
    ]
}

#[test]
fn a_program_that_never_reads_its_input_is_not_a_failure() {
    let dir = TempDir::new("no-read");
    let path = executable(&dir, &syscall(60, 3), vec![], 0);
    if let Some(ran) = run(&path, &vec![b'x'; MIB]) {
        assert_eq!(ran.unwrap(), (3, vec![], vec![]));
    }
}

#[test]
fn a_program_that_writes_a_mebibyte_before_reading_does_not_deadlock() {
    let dir = TempDir::new("flood");
    let mut xs = vec![
        X::MovRI(R::Rax, 1),
        X::MovRI(R::Rdi, 1),
        X::Lea(R::Rsi, Mem::Rip(Sym::Text(0))),
        X::MovRI(R::Rdx, MIB as i64),
        X::Syscall,
        X::MovRI(R::Rax, 0),
        X::MovRI(R::Rdi, 0),
        X::Lea(R::Rsi, Mem::Rip(Sym::Global(Global::OutBuf))),
        X::MovRI(R::Rdx, 4096),
        X::Syscall,
    ];
    xs.extend(syscall(60, 7));
    let path = executable(&dir, &xs, vec![b'y'; MIB], 4096);
    if let Some(ran) = run(&path, &vec![b'x'; MIB]) {
        let (status, out, err) = ran.unwrap();
        assert_eq!((status, out.len(), err.len()), (7, MIB, 0));
    }
}

#[test]
fn a_program_that_outlives_the_deadline_is_reported_as_timed_out() {
    let dir = TempDir::new("hang");
    // nanosleep(&{2 s, 0 ns}, null) then exit(0)
    let mut xs = vec![X::MovRI(R::Rax, 35), X::Lea(R::Rdi, Mem::Rip(Sym::Text(0)))];
    xs.extend([X::MovRI(R::Rsi, 0), X::Syscall]);
    xs.extend(syscall(60, 0));
    let mut ts = 2u64.to_le_bytes().to_vec();
    ts.extend([0; 8]);
    let path = executable(&dir, &xs, ts, 0);
    if let Some(ran) = run_within(&path, b"", Duration::from_millis(300)) {
        assert!(ran.unwrap_err().contains("timed out"));
    }
}

#[test]
fn a_program_killed_by_a_signal_is_reported_not_a_panic() {
    let dir = TempDir::new("signal");
    // kill(getpid(), 9)
    let mut xs = vec![X::MovRI(R::Rax, 39), X::Syscall, X::MovRR(R::Rdi, R::Rax)];
    xs.extend([X::MovRI(R::Rsi, 9), X::MovRI(R::Rax, 62), X::Syscall]);
    let path = executable(&dir, &xs, vec![], 0);
    if let Some(ran) = run(&path, b"") {
        match ran {
            Err(msg) => assert!(msg.contains("killed by signal 9"), "{msg}"),
            Ok((status, ..)) => {
                assert_ne!(status, 0, "a machine runner reports the kill as a status")
            }
        }
    }
}

fn writes_a_mebibyte(dir: &TempDir) -> std::path::PathBuf {
    let mut xs = vec![
        X::MovRI(R::Rax, 1),
        X::MovRI(R::Rdi, 1),
        X::Lea(R::Rsi, Mem::Rip(Sym::Text(0))),
        X::MovRI(R::Rdx, MIB as i64),
        X::Syscall,
    ];
    xs.extend(syscall(60, 0));
    executable(dir, &xs, vec![b'z'; MIB], 0)
}

#[test]
#[ignore = "stress: 1600 runs through the runner; cargo test -- --ignored"]
fn a_mebibyte_is_never_truncated_in_1600_runs() {
    let dir = TempDir::new("stress");
    let path = writes_a_mebibyte(&dir);
    if runner().is_none() {
        run_support::skip();
        return;
    }
    let short: usize = std::thread::scope(|s| {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                s.spawn(|| {
                    (0..200)
                        .filter(|_| {
                            let (_, out, _) = run(&path, b"").unwrap().unwrap();
                            out.len() != MIB
                        })
                        .count()
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).sum()
    });
    println!("1600 runs, {short} short");
    assert_eq!(short, 0);
}

#[test]
fn under_orb_stdout_stderr_and_status_come_back_exactly() {
    if runner().and_then(|r| r.first().cloned()).as_deref() != Some("orb") {
        return run_support::skip();
    }
    let dir = TempDir::new("orb-capture");
    // write(1, "out!", 4); write(2, "err??", 5); exit(5)
    let xs = vec![
        X::MovRI(R::Rax, 1),
        X::MovRI(R::Rdi, 1),
        X::Lea(R::Rsi, Mem::Rip(Sym::Text(0))),
        X::MovRI(R::Rdx, 4),
        X::Syscall,
        X::MovRI(R::Rax, 1),
        X::MovRI(R::Rdi, 2),
        X::Lea(R::Rsi, Mem::Rip(Sym::Text(0))),
        X::AluI(AluOp::Add, R::Rsi, 4),
        X::MovRI(R::Rdx, 5),
        X::Syscall,
        X::MovRI(R::Rax, 60),
        X::MovRI(R::Rdi, 5),
        X::Syscall,
    ];
    let path = executable(&dir, &xs, b"out!err??".to_vec(), 0);
    assert_eq!(
        run(&path, b"").unwrap().unwrap(),
        (5, b"out!".to_vec(), b"err??".to_vec())
    );
}

#[test]
fn a_missing_output_file_is_an_error_naming_it_not_empty_output() {
    let dir = TempDir::new("read-back");
    let (o, e) = (dir.0.join("o"), dir.0.join("e"));
    std::fs::write(&o, b"a").unwrap();
    let msg = read_back(&o, &e).unwrap_err();
    assert!(msg.contains(e.to_str().unwrap()), "{msg}");
    std::fs::write(&e, b"b").unwrap();
    assert_eq!(read_back(&o, &e).unwrap(), (b"a".to_vec(), b"b".to_vec()));
}
