//! The Linux system-call primitives. The arguments were pushed left to right, so they are
//! popped in reverse into the syscall registers. The kernel clobbers `rcx` (`b`) and `r11`:
//! `rcx` is saved around the call, `r11` is scratch.

use super::inst::{AluOp, Cc, Mem, R, X};
use crate::ir::{OpenMode, Prim};

const READ: i64 = 0;
const WRITE: i64 = 1;
const MMAP: i64 = 9;
const GETRLIMIT: i64 = 97;
const EXIT_GROUP: i64 = 231;
const RLIMIT_STACK: i64 = 3;
const PROT_READ_WRITE: i64 = 3;
const MAP_PRIVATE_ANONYMOUS: i64 = 0x22;
const CLOSE: i64 = 3;
const FSYNC: i64 = 74;
const FSTAT: i64 = 5;
const RENAMEAT: i64 = 264;
const OPENAT2: i64 = 437;
const O_RDONLY: i64 = 0;
const O_WRONLY: i64 = 1;
const O_CREAT: i64 = 0o100;
const O_TRUNC: i64 = 0o1000;
const O_DIRECTORY: i64 = 0o200000;
const O_CLOEXEC: i64 = 0o2000000;
const O_PATH: i64 = 0o10000000;
const RESOLVE_NO_SYMLINKS: i64 = 0x04;
const RESOLVE_BENEATH: i64 = 0x08;
/// The permissions of a created file.
const CREATE_PERMISSIONS: i64 = 0o644;
/// `sizeof(struct open_how)`: `u64` flags, mode and resolve.
const OPEN_HOW_SIZE: i64 = 24;
/// `sizeof(struct stat)` on x86-64, and where `st_mode` sits in it.
const STAT_SIZE: i32 = 144;
const ST_MODE: i32 = 24;
/// The permission bits of `st_mode`, with setuid, setgid and sticky.
const PERMISSION_BITS: i32 = 0o7777;
/// The kernel reports an error as a result in `-4095..=-1`.
const FIRST_ERROR: i32 = -4096;

/// Lowers one primitive. `unique` makes this function's labels distinct from every other.
pub fn system(prim: Prim, unique: &str, out: &mut Vec<X>) {
    match prim {
        Prim::Write => transfer(WRITE, out),
        Prim::Read => transfer(READ, out),
        Prim::Exit => out.extend([X::Pop(R::Rdi), X::MovRI(R::Rax, EXIT_GROUP), X::Syscall]),
        Prim::Map => map(unique, out),
        Prim::StackSize => stack_size(out),
        Prim::Open(mode) => open(mode, out),
        Prim::Close => single(CLOSE, out),
        Prim::Sync => single(FSYNC, out),
        Prim::Mode => mode(unique, out),
        Prim::Rename => rename(out),
    }
}

/// `syscall` with `rcx` kept.
fn call(number: i64, out: &mut Vec<X>) {
    out.extend([
        X::MovRI(R::Rax, number),
        X::Push(R::Rcx),
        X::Syscall,
        X::Pop(R::Rcx),
    ]);
}

fn transfer(number: i64, out: &mut Vec<X>) {
    out.extend([X::Pop(R::Rdx), X::Pop(R::Rsi), X::Pop(R::Rdi)]);
    call(number, out);
}

/// `mmap(0, len, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0)`; any error,
/// including the kernel's refusal of a zero length, gives 0.
fn map(unique: &str, out: &mut Vec<X>) {
    let ok = format!("{unique}_mapped");
    out.extend([
        X::Pop(R::Rsi),
        X::MovRI(R::Rdi, 0),
        X::MovRI(R::Rdx, PROT_READ_WRITE),
        X::MovRI(R::R10, MAP_PRIVATE_ANONYMOUS),
        X::MovRI(R::R8, -1),
        X::MovRI(R::R9, 0),
    ]);
    call(MMAP, out);
    out.extend([
        X::AluI(AluOp::Cmp, R::Rax, FIRST_ERROR),
        X::Jcc(Cc::B, ok.clone()),
        X::Alu(AluOp::Xor, R::Rax, R::Rax),
        X::Label(ok),
    ]);
}

/// `getrlimit(RLIMIT_STACK, buf)` into 16 bytes of stack; `RLIM_INFINITY` is `u64::MAX`,
/// which is already -1.
fn stack_size(out: &mut Vec<X>) {
    out.extend([
        X::Push(R::Rcx),
        X::AluI(AluOp::Sub, R::Rsp, 16),
        X::MovRI(R::Rdi, RLIMIT_STACK),
        X::MovRR(R::Rsi, R::Rsp),
        X::MovRI(R::Rax, GETRLIMIT),
        X::Syscall,
        X::MovRM(R::Rax, Mem::Base(R::Rsp, 0)),
        X::AluI(AluOp::Add, R::Rsp, 16),
        X::Pop(R::Rcx),
    ]);
}

/// `openat2(folder, path, &open_how, 24)`, always `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`.
/// The struct is built on the stack (flags at the lowest address) before `call` saves `rcx`,
/// so `rdx` is its address; it is released after `rcx` comes back, and `rax` is untouched.
fn open(mode: OpenMode, out: &mut Vec<X>) {
    let (flags, permissions) = match mode {
        OpenMode::Read => (O_RDONLY | O_CLOEXEC, 0),
        OpenMode::Folder => (O_RDONLY | O_DIRECTORY | O_CLOEXEC, 0),
        OpenMode::Create => (O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, CREATE_PERMISSIONS),
        OpenMode::Probe => (O_PATH | O_CLOEXEC, 0),
    };
    out.extend([X::Pop(R::Rsi), X::Pop(R::Rdi)]);
    for field in [RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS, permissions, flags] {
        out.extend([X::MovRI(R::R11, field), X::Push(R::R11)]);
    }
    out.extend([X::MovRR(R::Rdx, R::Rsp), X::MovRI(R::R10, OPEN_HOW_SIZE)]);
    call(OPENAT2, out);
    out.push(X::AluI(AluOp::Add, R::Rsp, OPEN_HOW_SIZE as i32));
}

/// A system call of one descriptor argument.
fn single(number: i64, out: &mut Vec<X>) {
    out.push(X::Pop(R::Rdi));
    call(number, out);
}

/// `fstat(fd, &stat)` into `sizeof(struct stat)` bytes of stack; the permission bits are the
/// low 12 bits of `st_mode`, at offset 24. An error is passed on as it is.
fn mode(unique: &str, out: &mut Vec<X>) {
    let failed = format!("{unique}_failed");
    out.extend([
        X::Pop(R::Rdi),
        X::AluI(AluOp::Sub, R::Rsp, STAT_SIZE),
        X::MovRR(R::Rsi, R::Rsp),
    ]);
    call(FSTAT, out);
    out.extend([
        X::AluI(AluOp::Cmp, R::Rax, FIRST_ERROR),
        X::Jcc(Cc::Ae, failed.clone()),
        X::MovRM(R::Rax, Mem::Base(R::Rsp, ST_MODE)),
        X::AluI(AluOp::And, R::Rax, PERMISSION_BITS),
        X::Label(failed),
        X::AluI(AluOp::Add, R::Rsp, STAT_SIZE),
    ]);
}

/// `renameat(folder, from, folder, to)`.
fn rename(out: &mut Vec<X>) {
    out.extend([
        X::Pop(R::R10),
        X::Pop(R::Rsi),
        X::Pop(R::Rdi),
        X::MovRR(R::Rdx, R::Rdi),
    ]);
    call(RENAMEAT, out);
}
