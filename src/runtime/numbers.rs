//! Showing numbers: `number_text` and the routines that write it.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Gt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp};

/// The room `number_text` allocates: the length, then at most 21 bytes (`-`, 15 whole
/// digits, `.` and 4 fraction digits), rounded up.
const TEXT_BYTES: i64 = 32;
/// The largest power of ten that fits an `i64`: the first digit of the whole number.
const FIRST_POWER: i64 = 1_000_000_000_000_000_000;
/// The power of ten of the units digit: the number is in ten-thousandths.
const UNITS: i64 = 10_000;

// Slots of `number_text`.
const BUF: u32 = 1;
const LEN: u32 = 2;
const REST: u32 = 3;
const POWER: u32 = 4;
const BYTE: u32 = 5;
const SHOWN: u32 = 6;
const DIGIT: u32 = 7;

/// Appends the byte in `a` to the text being built (clobbers `a` and `b`).
fn put(s: &mut Asm) {
    s.i(StoreSlot(BYTE, A));
    s.i(LoadSlot(A, BUF));
    s.i(LoadSlot(B, LEN));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, BYTE));
    s.i(StoreByte {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(LoadSlot(A, LEN));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(LEN, A));
}

/// `number_text(n, line)`: the show form of `n`: `-` for a negative number, the whole part,
/// then `.` and the fraction without its trailing zeros when it is not zero. One pass over
/// the digits of `n` from the highest power of ten, as `err_int` does, on `-|n|` so that
/// `i64::MIN` needs no special case. Leading zeros are skipped up to the units digit; the
/// pass ends when nothing is left at or after the units digit.
fn number_text() -> Function {
    let mut s = Asm::default();
    let (positive, store, again, emit, skip, next, done) = (
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
    );
    s.push_const(TEXT_BYTES);
    s.push_arg(2);
    s.call("alloc");
    s.i(StoreSlot(BUF, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(LEN, A));
    s.i(StoreSlot(SHOWN, A));
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Ge, positive);
    s.i(Const(A, i64::from(b'-')));
    put(&mut s);
    s.i(LoadArg(A, 1));
    s.i(Jump(store));
    s.i(Mark(positive));
    s.i(Move { to: B, from: A });
    s.i(Const(A, 0));
    s.i(Int(IntOp::Sub));
    s.i(Mark(store));
    s.i(StoreSlot(REST, A));
    s.i(Const(A, FIRST_POWER));
    s.i(StoreSlot(POWER, A));
    s.i(Mark(again));
    for op in [IntOp::Div, IntOp::Rem] {
        s.i(LoadSlot(A, REST));
        s.i(LoadSlot(B, POWER));
        s.i(Int(op));
        s.i(StoreSlot(if op == IntOp::Div { BYTE } else { REST }, A));
    }
    s.i(Const(A, 0));
    s.i(LoadSlot(B, BYTE));
    s.i(Int(IntOp::Sub));
    s.i(StoreSlot(DIGIT, A));
    s.i(Const(B, 0));
    s.jump_if(Ne, emit);
    s.i(LoadSlot(A, SHOWN));
    s.jump_if(Ne, emit);
    s.i(LoadSlot(A, POWER));
    s.i(Const(B, UNITS));
    s.jump_if(Ne, skip);
    s.i(Mark(emit));
    s.i(Const(A, 1));
    s.i(StoreSlot(SHOWN, A));
    s.i(LoadSlot(A, DIGIT));
    s.i(AddImm(A, i32::from(b'0')));
    put(&mut s);
    s.i(Mark(skip));
    s.i(LoadSlot(A, POWER));
    s.i(Const(B, UNITS));
    s.jump_if(Gt, next);
    s.i(LoadSlot(A, REST));
    s.i(Const(B, 0));
    s.jump_if(Eq, done);
    s.i(LoadSlot(A, POWER));
    s.i(Const(B, UNITS));
    s.jump_if(Ne, next);
    s.i(Const(A, i64::from(b'.')));
    put(&mut s);
    s.i(Mark(next));
    s.i(LoadSlot(A, POWER));
    s.i(Const(B, 10));
    s.i(Int(IntOp::Div));
    s.i(StoreSlot(POWER, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(LoadSlot(A, BUF));
    s.i(LoadSlot(B, LEN));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(Return);
    s.finish("number_text", 2, 7)
}

pub fn routines() -> Vec<Function> {
    vec![number_text()]
}
