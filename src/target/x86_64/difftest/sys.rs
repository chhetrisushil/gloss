//! Programs for every system primitive. Each writes the primitive's result and `b` (set
//! beforehand, so a clobbered `b` shows) before anything else, then exits.

use super::{program, write_text, write_words};
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Function, Global, Inst, Label, Module, Prim};

pub const B_VALUE: i64 = 0x1111_2222_3333;

fn out_buf(offset: i32) -> Vec<Inst> {
    vec![
        Inst::AddressOf(A, Data::Global(Global::OutBuf)),
        Inst::AddImm(A, offset),
        Inst::Push(A),
    ]
}

fn push(v: i64) -> Vec<Inst> {
    vec![Inst::Const(A, v), Inst::Push(A)]
}

/// Runs `args` and the primitive, then writes the result and `b`, and exits 0.
fn finish(m: &mut Module, args: Vec<Inst>, prim: Prim, tail: Vec<Inst>) {
    let mut body = vec![Inst::Const(B, B_VALUE)];
    body.extend(args);
    body.push(Inst::System(prim));
    body.extend([Inst::Push(B), Inst::Push(A)]);
    body.extend(write_words(2));
    body.extend(tail);
    body.extend(push(0));
    body.push(Inst::System(Prim::Exit));
    program(m, body);
}

fn write_to(fd: i64) -> Module {
    let mut m = Module::default();
    let text = m.text(b"hello");
    let mut args = push(fd);
    args.extend([
        Inst::AddressOf(A, Data::Text(text)),
        Inst::AddImm(A, 8),
        Inst::Push(A),
    ]);
    args.extend(push(5));
    finish(&mut m, args, Prim::Write, Vec::new());
    m
}

/// Reads `len` bytes of standard input (from descriptor `fd`) into `out_buf`, then writes
/// the 5 bytes at its start after the count and `b`.
fn read_from(fd: i64, len: i64) -> Module {
    let mut m = Module::default();
    let mut args = push(fd);
    args.extend(out_buf(0));
    args.extend(push(len));
    let mut tail = out_buf(0);
    tail.insert(0, Inst::Const(A, 1));
    tail.insert(1, Inst::Push(A));
    tail.extend(push(5));
    tail.push(Inst::System(Prim::Write));
    finish(&mut m, args, Prim::Read, tail);
    m
}

/// `Map(len)`: writes whether it succeeded, then values stored and loaded back.
fn map(len: i64) -> Module {
    let mut m = Module::default();
    let mut body = vec![Inst::Const(B, B_VALUE)];
    body.extend(push(len));
    body.extend([
        Inst::System(Prim::Map),
        Inst::Push(B),
        Inst::Move { to: B, from: A },
        Inst::Const(A, 0),
        Inst::Compare,
        Inst::SetIf(Cond::Ne),
        Inst::Push(A),
    ]);
    if len >= 8192 {
        body.extend([
            Inst::Const(A, 0x1234),
            Inst::Store {
                base: B,
                offset: 0,
                from: A,
            },
            Inst::Const(A, 0x5678),
            Inst::Store {
                base: B,
                offset: 8184,
                from: A,
            },
        ]);
        for offset in [0, 4096, 8184] {
            body.extend([
                Inst::Load {
                    to: A,
                    base: B,
                    offset,
                },
                Inst::Push(A),
            ]);
        }
    }
    body.extend(write_words(if len >= 8192 { 5 } else { 2 }));
    body.extend(push(0));
    body.push(Inst::System(Prim::Exit));
    program(&mut m, body);
    m
}

fn stack_size() -> Module {
    let mut m = Module::default();
    let mut body = vec![
        Inst::Const(B, B_VALUE),
        Inst::System(Prim::StackSize),
        Inst::Push(B),
        Inst::Const(B, 0),
        Inst::Compare,
        Inst::SetIf(Cond::Gt),
        Inst::Push(A),
    ];
    body.extend(write_words(2));
    body.extend(push(0));
    body.push(Inst::System(Prim::Exit));
    program(&mut m, body);
    m
}

/// Writes the text `out`, then exits with `status`.
fn exit_with(status: i64) -> Module {
    let mut m = Module::default();
    let text = m.text(b"out");
    let mut body = write_text(text, 3);
    body.extend(push(status));
    body.push(Inst::System(Prim::Exit));
    program(&mut m, body);
    m
}

/// Forks; the child runs `child`, which must exit. The parent, with `b` set before the fork
/// and a word pushed before the wait, waits for the child and writes the status, `b` and the
/// word: `b` surviving both calls and `rsp` being as it was show in the output.
pub fn fork_wait(child: &[Inst]) -> Module {
    let mut body = vec![
        Inst::Enter(2),
        Inst::Const(B, B_VALUE),
        Inst::System(Prim::Fork),
        Inst::StoreSlot(1, A),
        Inst::StoreSlot(2, B),
        Inst::Const(B, 0),
        Inst::Compare,
        Inst::JumpIf(Cond::Eq, Label(1)),
        Inst::LoadSlot(B, 2),
    ];
    body.extend(push(0x5eed));
    body.extend([
        Inst::LoadSlot(A, 1),
        Inst::Push(A),
        Inst::System(Prim::Wait),
        Inst::Push(B),
        Inst::Push(A),
    ]);
    body.extend(write_words(3));
    body.extend(push(0));
    body.push(Inst::System(Prim::Exit));
    body.push(Inst::Mark(Label(1)));
    body.extend(child.iter().cloned());
    let mut m = Module::default();
    m.functions.push(Function {
        name: "_start".into(),
        params: 0,
        slots: 2,
        body,
    });
    m
}

/// A child that exits with `status`.
pub fn child_exits(status: i64) -> Vec<Inst> {
    let mut body = push(status);
    body.push(Inst::System(Prim::Exit));
    body
}

/// A child that stores through a null pointer: Linux ends it with `SIGSEGV`, which irsim
/// does not model.
pub fn child_faults() -> Vec<Inst> {
    vec![
        Inst::Const(A, 0),
        Inst::Store {
            base: A,
            offset: 0,
            from: A,
        },
    ]
}

/// `Wait` for a pid that is no child of this process (above Linux's `pid_max`).
fn wait_stranger() -> Module {
    let mut m = Module::default();
    finish(&mut m, push(99_999_999), Prim::Wait, Vec::new());
    m
}

/// `DefaultChildSignal`: 0 and `b` kept, on both.
fn default_child_signal() -> Module {
    let mut m = Module::default();
    finish(&mut m, Vec::new(), Prim::DefaultChildSignal, Vec::new());
    m
}

/// Calls `each` with the name, module and standard input of every program.
pub fn programs(each: &mut dyn FnMut(&str, &Module, &[u8])) {
    each("write out", &write_to(1), b"");
    each("write err", &write_to(2), b"");
    each("write bad fd", &write_to(77), b"");
    each("read all", &read_from(0, 4096), b"hello world");
    each("read some", &read_from(0, 3), b"hello world");
    each("read empty", &read_from(0, 4096), b"");
    each("read bad fd", &read_from(77, 4096), b"hello");
    for len in [8192, 0, -1] {
        each(&format!("map {len}"), &map(len), b"");
    }
    each("stack size", &stack_size(), b"");
    for status in [0, 7, 255, 256, 257, -1] {
        each(&format!("exit {status}"), &exit_with(status), b"");
    }
    for status in [0, 3, 255] {
        each(
            &format!("fork exit {status}"),
            &fork_wait(&child_exits(status)),
            b"",
        );
    }
    each("wait stranger", &wait_stranger(), b"");
    each("default child signal", &default_child_signal(), b"");
}
