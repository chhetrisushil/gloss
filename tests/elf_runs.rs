//! A hand-assembled image runs on Linux: the exit status comes from read-only data, after
//! a round trip through the data segment.

mod run_support;

use gloss::ir::Global;
use gloss::target::x86_64::inst::{Mem, R, Sym, X};
use run_support::{TempDir, executable, run};

#[test]
fn a_value_from_rodata_stored_in_data_and_read_back_becomes_the_exit_status() {
    let xs = [
        X::MovRI(R::Rax, 60),
        X::MovRM(R::Rdi, Mem::Rip(Sym::Text(0))),
        X::MovMR(Mem::Rip(Sym::Global(Global::OutBuf)), R::Rdi),
        X::MovRI(R::Rdi, 0),
        X::MovRM(R::Rdi, Mem::Rip(Sym::Global(Global::OutBuf))),
        X::Syscall,
    ];
    let dir = TempDir::new("exit42");
    let path = executable(&dir, &xs, 42u64.to_le_bytes().to_vec(), 0);
    if let Some(ran) = run(&path, b"") {
        assert_eq!(ran.unwrap(), (42, Vec::new(), Vec::new()));
    }
}
