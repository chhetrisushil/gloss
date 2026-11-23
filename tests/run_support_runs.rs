//! The runner copes with programs that ignore stdin, flood stdout, hang or are killed.

mod run_support;

use gloss::target::x86_64::inst::{R, X};
use run_support::{TempDir, executable, run};

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
