//! The routines a trace build swaps in for the real ones: `out_bytes`, and the failure
//! routines, which turn a run-time failure into one `fail` event on standard output. Later
//! tasks add the event writers and the replaced input routines.
//!
//! Text enters the output buffer through `out_bytes`; a trace build's one decides, by the
//! trace state's `ESCAPING` word, whether it is JSON-escaped on the way (`json_bytes`, in
//! `json.rs`).

use super::asm::Asm;
use super::trace_state::{CARRY, ESCAPING, FAILING, OPEN, get, set};
use super::values::out_const;
use super::{FAIL_CLOSE, FAIL_EVENT, FAIL_LINE, FAIL_MESSAGE, NEWLINE};
use crate::ir::Cond::{Eq, Ge, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Prim};

/// Names of the real routines a trace build replaces.
pub const REPLACED: [&str; 4] = ["out_bytes", "write_all", "fail_begin", "fail_end"];

/// Stores the constant `n` in the trace state word at `offset`.
fn put_word(s: &mut Asm, offset: i32, n: i64) {
    s.i(Const(A, n));
    set(s, offset, A, B);
}

/// `write_all(fd, addr, len)`: descriptor 2 while `FAILING` is the failure message, which goes
/// into the event through `fail_piece` (escaped, a cut character kept whole); descriptor 2
/// otherwise is a notice, so the events pending reach standard output first; anything else
/// takes the real loop.
fn write_all() -> Function {
    let mut s = Asm::default();
    let (real, failing) = (s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(Const(B, 2));
    s.jump_if(Ne, real);
    get(&mut s, A, FAILING);
    s.i(Const(B, 0));
    s.jump_if(Ne, failing);
    s.call("flush");
    s.i(Mark(real));
    s.push_arg(1);
    s.push_arg(2);
    s.push_arg(3);
    s.call("write_loop");
    s.i(Return);
    s.i(Mark(failing));
    s.push_arg(2);
    s.push_arg(3);
    s.call("fail_piece");
    s.finish("write_all", 3, 0)
}

/// Stores `from` as the byte at `offset` past `CARRY`, with the state's address in `B`.
fn carry_byte(s: &mut Asm, offset: i32, from: crate::ir::Reg) {
    s.i(AddressOf(B, Data::Global(Global::TraceState)));
    s.i(StoreByte {
        base: B,
        offset: CARRY + offset,
        from,
    });
}

/// `a` = the byte at `offset` past `CARRY`.
fn load_carry_byte(s: &mut Asm, offset: i32) {
    s.i(AddressOf(A, Data::Global(Global::TraceState)));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: CARRY + offset,
    });
}

/// `carry_emit()`: writes the carried bytes, if any, through `out_bytes` (what is not a whole
/// character becomes `U+FFFD` there) and empties the carry. Slot 1: the count.
fn carry_emit() -> Function {
    let mut s = Asm::default();
    let none = s.label();
    load_carry_byte(&mut s, 4);
    s.i(StoreSlot(1, A));
    s.i(Const(B, 0));
    s.jump_if(Eq, none);
    s.i(AddressOf(A, Data::Global(Global::TraceState)));
    s.i(AddImm(A, CARRY));
    s.i(Push(A));
    s.push_slot(1);
    s.call("out_bytes");
    s.i(Const(A, 0));
    carry_byte(&mut s, 4, A);
    s.i(Mark(none));
    s.finish("carry_emit", 0, 1)
}

/// `fail_piece(addr, len)`: a piece of the failure message, through `out_bytes` a byte at a
/// time; a character cut at the end waits in the carry for the next piece (`err_put` sends
/// 24 bytes at a time). A lead byte (0xC0 to 0xF7) starts the carry, a continuation byte
/// (0x80 to 0xBF) joins it and the character goes out when whole; any other byte writes the
/// carry as it is first. Slots: 1 the index, 2 the byte, 3 the carry's count, 4 the
/// character's length.
fn fail_piece() -> Function {
    let mut s = Asm::default();
    let [again, done, fresh, plain, next, broken] = [(); 6].map(|_| s.label());
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadArg(B, 2));
    s.jump_if(Ge, done);
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(2, A));
    load_carry_byte(&mut s, 4);
    s.i(StoreSlot(3, A));
    s.i(Const(B, 0));
    s.jump_if(Eq, fresh);
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0x80));
    s.jump_if(Lt, broken);
    s.i(Const(B, 0xC0));
    s.jump_if(Ge, broken);
    s.i(AddressOf(A, Data::Global(Global::TraceState)));
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, 2));
    s.i(StoreByte {
        base: A,
        offset: CARRY,
        from: B,
    });
    s.i(LoadSlot(A, 3));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(3, A));
    carry_byte(&mut s, 4, A);
    load_carry_byte(&mut s, 5);
    s.i(LoadSlot(B, 3));
    s.jump_if(Ne, next);
    s.call("carry_emit");
    s.i(Jump(next));
    s.i(Mark(broken));
    s.call("carry_emit");
    s.i(Mark(fresh));
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0xC0));
    s.jump_if(Lt, plain);
    s.i(Const(B, 0xF8));
    s.jump_if(Ge, plain);
    s.i(LoadSlot(A, 2));
    carry_byte(&mut s, 0, A);
    s.i(Const(A, 1));
    carry_byte(&mut s, 4, A);
    // The length of the character: 2 below 0xE0, 3 below 0xF0, else 4.
    s.i(Const(A, 2));
    s.i(StoreSlot(4, A));
    for (limit, len) in [(0xE0, 3), (0xF0, 4)] {
        let stay = s.label();
        s.i(LoadSlot(A, 2));
        s.i(Const(B, limit));
        s.jump_if(Lt, stay);
        s.i(Const(A, len));
        s.i(StoreSlot(4, A));
        s.i(Mark(stay));
    }
    s.i(LoadSlot(A, 4));
    carry_byte(&mut s, 5, A);
    s.i(Jump(next));
    s.i(Mark(plain));
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(Push(A));
    s.push_const(1);
    s.call("out_bytes");
    s.i(Mark(next));
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("fail_piece", 2, 4)
}

/// `fail_begin(line, code)`: closes an open `show` event (`"}` and a newline), then writes
/// `{"line":<line>,"event":"fail","failure":<code>,"message":"` and starts `FAILING`, with
/// the message escaped.
fn fail_begin() -> Function {
    let mut s = Asm::default();
    let closed = s.label();
    put_word(&mut s, ESCAPING, 0);
    get(&mut s, A, OPEN);
    s.i(Const(B, 0));
    s.jump_if(Eq, closed);
    out_const(&mut s, FAIL_CLOSE);
    put_word(&mut s, OPEN, 0);
    s.i(Mark(closed));
    out_const(&mut s, FAIL_LINE);
    s.push_arg(1);
    s.call("json_int");
    out_const(&mut s, FAIL_EVENT);
    s.push_arg(2);
    s.call("json_int");
    out_const(&mut s, FAIL_MESSAGE);
    put_word(&mut s, FAILING, 1);
    put_word(&mut s, ESCAPING, 1);
    s.finish("fail_begin", 2, 0)
}

/// `fail_end()`: writes a cut character that never finished, ends the message (`"}` and a
/// newline), flushes and exits with status 2. With no `fail_begin` before it (`FAILING` is 0:
/// the internal error of a feature that ends without giving) it is the real one: a newline
/// on descriptor 2 and exit 2, no event.
fn fail_end() -> Function {
    let mut s = Asm::default();
    let failing = s.label();
    get(&mut s, A, FAILING);
    s.i(Const(B, 0));
    s.jump_if(Ne, failing);
    s.push_text(NEWLINE);
    s.call("err_text");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.i(Mark(failing));
    s.call("carry_emit");
    put_word(&mut s, ESCAPING, 0);
    out_const(&mut s, FAIL_CLOSE);
    s.call("flush");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.finish("fail_end", 0, 0)
}

/// `out_bytes(addr, len)`: `buf_bytes` while not `ESCAPING`, else `json_bytes`.
fn out_bytes() -> Function {
    let mut s = Asm::default();
    let escape = s.label();
    get(&mut s, A, ESCAPING);
    s.i(Const(B, 0));
    s.jump_if(Ne, escape);
    s.push_arg(1);
    s.push_arg(2);
    s.call("buf_bytes");
    s.i(Return);
    s.i(Mark(escape));
    s.push_arg(1);
    s.push_arg(2);
    s.call("json_bytes");
    s.finish("out_bytes", 2, 0)
}

/// The routines a trace build adds.
pub fn routines() -> Vec<Function> {
    vec![
        out_bytes(),
        write_all(),
        carry_emit(),
        fail_piece(),
        fail_begin(),
        fail_end(),
    ]
}
