//! Reading standard input through `in_buf`.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Le, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Prim};

/// The size of `Global::InBuf`.
const IN_BUF: i64 = crate::ir::IO_BUF_BYTES as i64;
/// How many bytes a line has room for before its first growth.
const FIRST_ROOM: i64 = 64;

/// `in_fill()`: 1 when `in_buf` has an unread byte at `in_pos`, else 0 at end of input. When
/// `in_pos` has reached `in_len` it reads up to `in_buf`'s size afresh; it never keeps or
/// looks at more than `Read` gave. A `Read` that gives less than 0 ends the program with
/// status 2 and no message, after flushing, as a failed write does (M2-R32).
fn in_fill() -> Function {
    let mut s = Asm::default();
    let (ready, eof, error) = (s.label(), s.label(), s.label());
    s.gload(A, Global::InPos);
    s.gload(B, Global::InLen);
    s.jump_if(Lt, ready);
    s.push_const(0);
    s.i(AddressOf(A, Data::Global(Global::InBuf)));
    s.i(Push(A));
    s.push_const(IN_BUF);
    s.i(System(Prim::Read));
    s.i(Const(B, 0));
    s.jump_if(Lt, error);
    s.jump_if(Eq, eof);
    s.gstore(Global::InLen, A, B);
    s.i(Const(A, 0));
    s.gstore(Global::InPos, A, B);
    s.i(Mark(ready));
    s.i(Const(A, 1));
    s.i(Return);
    s.i(Mark(eof));
    s.i(Const(A, 0));
    s.i(Return);
    s.i(Mark(error));
    s.call("flush");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.finish("in_fill", 0, 0)
}

// Slots of `ask`: the line's text, its room in bytes, its length, whether any byte (a line
// ending included) was read, the byte at hand, and, while growing, the new text, its room
// and how much is copied.
const TEXT: u32 = 1;
const ROOM: u32 = 2;
const LEN: u32 = 3;
const ANY: u32 = 4;
const BYTE: u32 = 5;

/// Adds `n` to the slot.
fn bump(s: &mut Asm, slot: u32, n: i32) {
    s.i(LoadSlot(A, slot));
    s.i(AddImm(A, n));
    s.i(StoreSlot(slot, A));
}

/// `a = slot x + slot y`.
fn sum(s: &mut Asm, x: u32, y: u32) {
    s.i(LoadSlot(A, x));
    s.i(LoadSlot(B, y));
    s.i(Int(IntOp::Add));
}

/// Stores `n` in each slot.
fn put(s: &mut Asm, slots: &[u32], n: i64) {
    s.i(Const(A, n));
    for &slot in slots {
        s.i(StoreSlot(slot, A));
    }
}

/// `a = byte at [a + offset]`.
fn load_byte(s: &mut Asm, offset: i32) {
    s.i(LoadByte {
        to: A,
        base: A,
        offset,
    });
}

/// `[a + 8] = b`, as a byte.
fn store_byte(s: &mut Asm) {
    s.i(StoreByte {
        base: A,
        offset: 8,
        from: B,
    });
}

/// `ask(question, line)`: shows the question on its own line, flushes, and reads one line,
/// whose text it gives. The bytes come from `in_buf`, refilled by `in_fill` (which ends the
/// program on a read error); the line is built in an allocated text that doubles when full, so
/// the bytes are never trusted to be short or to be UTF-8. A `\n` ends the line and is dropped,
/// and so is a `\r` just before it, wherever the refills fall. The spec removes only a line
/// ending, so a last line with no `\n` is kept whole, a final `\r` included. End of input
/// before any byte is failure 9; a line of only `\n` is an empty text, not that failure.
fn ask() -> Function {
    let mut s = Asm::default();
    let (next, room) = (s.label(), s.label());
    let (ending, finish, eof) = (s.label(), s.label(), s.label());
    s.push_arg(1);
    s.call("out_text");
    s.push_text(super::NEWLINE);
    s.call("out_text");
    s.call("flush");
    s.push_const(8 + FIRST_ROOM);
    s.push_arg(2);
    s.call("alloc");
    s.i(StoreSlot(TEXT, A));
    put(&mut s, &[ROOM], FIRST_ROOM);
    put(&mut s, &[LEN, ANY], 0);
    s.i(Mark(next));
    s.call("in_fill");
    s.i(Const(B, 0));
    s.jump_if(Eq, eof);
    s.gload(A, Global::InPos);
    s.i(AddressOf(B, crate::ir::Data::Global(Global::InBuf)));
    s.i(Int(IntOp::Add));
    load_byte(&mut s, 0);
    s.i(StoreSlot(BYTE, A));
    s.gload(A, Global::InPos);
    s.i(AddImm(A, 1));
    s.gstore(Global::InPos, A, B);
    put(&mut s, &[ANY], 1);
    s.i(LoadSlot(A, BYTE));
    s.i(Const(B, 10));
    s.jump_if(Eq, ending);
    s.i(LoadSlot(A, LEN));
    s.i(LoadSlot(B, ROOM));
    s.jump_if(Lt, room);
    // Full: a text twice as big, with the bytes so far copied over.
    s.i(LoadSlot(A, TEXT));
    s.i(Push(A));
    for slot in [LEN, ROOM] {
        s.i(LoadSlot(A, slot));
        s.i(Push(A));
    }
    s.push_arg(2);
    s.call("grow_text");
    s.i(StoreSlot(TEXT, A));
    s.i(LoadSlot(A, ROOM));
    s.i(Move { to: B, from: A });
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(ROOM, A));
    // Append the byte.
    s.i(Mark(room));
    sum(&mut s, TEXT, LEN);
    s.i(LoadSlot(B, BYTE));
    store_byte(&mut s);
    bump(&mut s, LEN, 1);
    s.i(Jump(next));
    // A `\n`: drop a `\r` before it.
    s.i(Mark(ending));
    s.i(LoadSlot(A, LEN));
    s.i(Const(B, 0));
    s.jump_if(Le, finish);
    sum(&mut s, TEXT, LEN);
    load_byte(&mut s, 7);
    s.i(Const(B, 13));
    s.jump_if(Ne, finish);
    bump(&mut s, LEN, -1);
    s.i(Mark(finish));
    s.i(LoadSlot(A, TEXT));
    s.i(LoadSlot(B, LEN));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(Return);
    s.i(Mark(eof));
    s.i(LoadSlot(A, ANY));
    s.i(Const(B, 0));
    s.jump_if(Ne, finish);
    s.push_arg(2);
    s.push_const(9);
    s.call("fail_begin");
    s.push_text(super::INPUT_ENDED);
    s.call("err_text");
    s.call("fail_end");
    s.i(Return);
    s.finish("ask", 2, 5)
}

pub fn routines() -> Vec<Function> {
    vec![in_fill(), ask()]
}
