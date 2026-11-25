//! The layout of `Global::TestState` and the routines that fill it. Each constant is a word
//! index (8 bytes a word) except `FAIL_CAP`. The words, from 0: the title, the shown buffer
//! (address, length, room), the answers (a list of text, or 0, and the index of the next),
//! `FAILING` (1 once the test is failing: reporting a failure must not allocate), the
//! failure text's length, and from word 8 its `FAIL_CAP` bytes. It is zero at the start.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Le, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Reg, Shift};

/// The address of the title text of the test being run.
pub const TITLE: u32 = 0;
pub const SHOWN: u32 = 1;
pub const SHOWN_LEN: u32 = 2;
pub const SHOWN_ROOM: u32 = 3;
pub const FAILING: u32 = 6;
pub const FAIL_LEN: u32 = 7;
/// The first of `FAIL_CAP` bytes of failure text.
pub const FAIL_BYTES: u32 = 8;
pub const FAIL_CAP: i64 = 1792;
pub const SHOWN_FIRST: i64 = 256;

/// Loads word `i` of the state into `to`.
fn get(s: &mut Asm, to: Reg, i: u32) {
    s.i(AddressOf(to, Data::Global(Global::TestState)));
    s.i(Load {
        to,
        base: to,
        offset: i as i32 * 8,
    });
}

/// Stores `from` into word `i` of the state, using `tmp` for the address.
fn set(s: &mut Asm, i: u32, from: Reg, tmp: Reg) {
    s.i(AddressOf(tmp, Data::Global(Global::TestState)));
    s.i(Store {
        base: tmp,
        offset: i as i32 * 8,
        from,
    });
}

/// `copy_bytes(src, dst, n)`: copies `n` bytes. Slot 1 is the index.
fn copy_bytes() -> Function {
    let mut s = Asm::default();
    let (next, done) = (s.label(), s.label());
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(next));
    s.i(LoadSlot(A, 1));
    s.i(LoadArg(B, 3));
    s.jump_if(Ge, done);
    s.i(LoadArg(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Push(A));
    s.i(LoadArg(A, 2));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(Pop(B));
    s.i(StoreByte {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(next));
    s.i(Mark(done));
    s.finish("copy_bytes", 3, 1)
}

/// `fail_add(addr, len)`: appends to the failure text, dropping what does not fit in
/// `FAIL_CAP` bytes. It never allocates, so a failure can be reported out of memory.
/// Slot 1: how many bytes are kept.
fn fail_add() -> Function {
    let mut s = Asm::default();
    let fits = s.label();
    s.i(LoadArg(A, 2));
    s.i(StoreSlot(1, A));
    s.i(Const(A, FAIL_CAP));
    get(&mut s, B, FAIL_LEN);
    s.i(Int(IntOp::Sub));
    s.i(LoadSlot(B, 1));
    s.jump_if(Ge, fits);
    s.i(StoreSlot(1, A));
    s.i(Mark(fits));
    s.push_arg(1);
    s.i(AddressOf(A, Data::Global(Global::TestState)));
    s.i(AddImm(A, FAIL_BYTES as i32 * 8));
    get(&mut s, B, FAIL_LEN);
    s.i(Int(IntOp::Add));
    s.i(Push(A));
    s.push_slot(1);
    s.call("copy_bytes");
    get(&mut s, A, FAIL_LEN);
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    set(&mut s, FAIL_LEN, A, B);
    s.finish("fail_add", 2, 1)
}

/// `shown_init()`: allocates the shown buffer, empty, with `SHOWN_FIRST` bytes of room.
fn shown_init() -> Function {
    let mut s = Asm::default();
    s.push_const(8 + SHOWN_FIRST);
    s.push_const(0);
    s.call("alloc");
    set(&mut s, SHOWN, A, B);
    s.i(Const(A, SHOWN_FIRST));
    set(&mut s, SHOWN_ROOM, A, B);
    s.finish("shown_init", 0, 0)
}

/// `shown_add(addr, len)`: appends to the shown bytes. The first one allocates the buffer
/// (`shown_init`) unless `FAILING`, a full one is grown to at least twice the room. While
/// `FAILING` it neither allocates nor grows: bytes that do not fit are dropped.
/// Slots: 1 the buffer, 2 the length it will have, 3 the room `grow_text` doubles.
fn shown_add() -> Function {
    let mut s = Asm::default();
    let (have, fits, doubled, done) = (s.label(), s.label(), s.label(), s.label());
    get(&mut s, A, SHOWN);
    s.i(StoreSlot(1, A));
    s.i(Const(B, 0));
    s.jump_if(Ne, have);
    get(&mut s, A, FAILING);
    s.jump_if(Ne, done);
    s.call("shown_init");
    get(&mut s, A, SHOWN);
    s.i(StoreSlot(1, A));
    s.i(Mark(have));
    get(&mut s, A, SHOWN_LEN);
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(2, A));
    get(&mut s, B, SHOWN_ROOM);
    s.jump_if(Le, fits);
    get(&mut s, A, FAILING);
    s.i(Const(B, 0));
    s.jump_if(Ne, done);
    get(&mut s, A, SHOWN_ROOM);
    s.i(StoreSlot(3, A));
    s.i(Shift(Shift::Left, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, doubled);
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(Shift(Shift::Right, 1));
    s.i(StoreSlot(3, A));
    s.i(Mark(doubled));
    s.push_slot(1);
    get(&mut s, A, SHOWN_LEN);
    s.i(Push(A));
    s.push_slot(3);
    s.push_const(0);
    s.call("grow_text");
    s.i(StoreSlot(1, A));
    set(&mut s, SHOWN, A, B);
    s.i(LoadSlot(A, 3));
    s.i(Shift(Shift::Left, 1));
    set(&mut s, SHOWN_ROOM, A, B);
    s.i(Mark(fits));
    s.push_arg(1);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 8));
    get(&mut s, B, SHOWN_LEN);
    s.i(Int(IntOp::Add));
    s.i(Push(A));
    s.push_arg(2);
    s.call("copy_bytes");
    s.i(LoadSlot(A, 2));
    set(&mut s, SHOWN_LEN, A, B);
    s.i(LoadSlot(B, 1));
    s.i(Store {
        base: B,
        offset: 0,
        from: A,
    });
    s.i(Mark(done));
    s.finish("shown_add", 2, 3)
}

pub fn routines() -> Vec<Function> {
    vec![copy_bytes(), fail_add(), shown_init(), shown_add()]
}
