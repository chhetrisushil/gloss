//! Converting a number to text with a fixed number of decimal places.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Le, Lt};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp};

// Slots of `put_decimal`.
const DIGITS: u32 = 1;
const COUNTER: u32 = 2;
const FROM: u32 = 3;
const BYTE: u32 = 4;
const REST: u32 = 5;

/// `put_decimal(buf, len, value, width)`: writes `value` (not negative) in decimal, at least
/// `width` digits, after the `len` bytes of the text being built in `buf`; returns the new
/// length. It counts the digits first, then writes them from the last one back.
fn put_decimal() -> Function {
    let mut s = Asm::default();
    let (count, counted, wide, write, done) =
        (s.label(), s.label(), s.label(), s.label(), s.label());
    s.i(Const(A, 1));
    s.i(StoreSlot(DIGITS, A));
    s.i(LoadArg(A, 3));
    s.i(StoreSlot(REST, A));
    s.i(StoreSlot(COUNTER, A));
    s.i(Mark(count));
    s.i(LoadSlot(A, COUNTER));
    s.i(Const(B, 10));
    s.jump_if(Lt, counted);
    s.i(Int(IntOp::Div));
    s.i(StoreSlot(COUNTER, A));
    s.i(LoadSlot(A, DIGITS));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(DIGITS, A));
    s.i(Jump(count));
    s.i(Mark(counted));
    s.i(LoadSlot(A, DIGITS));
    s.i(LoadArg(B, 4));
    s.jump_if(Ge, wide);
    s.i(StoreSlot(DIGITS, B));
    s.i(Mark(wide));
    s.i(LoadSlot(A, DIGITS));
    s.i(StoreSlot(FROM, A));
    s.i(Mark(write));
    s.i(LoadSlot(A, FROM));
    s.i(Const(B, 0));
    s.jump_if(Le, done);
    s.i(AddImm(A, -1));
    s.i(StoreSlot(FROM, A));
    s.i(LoadSlot(A, REST));
    s.i(Const(B, 10));
    s.i(Int(IntOp::Rem));
    s.i(AddImm(A, i32::from(b'0')));
    s.i(StoreSlot(BYTE, A));
    s.i(LoadSlot(A, REST));
    s.i(Const(B, 10));
    s.i(Int(IntOp::Div));
    s.i(StoreSlot(REST, A));
    s.i(LoadArg(A, 1));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, FROM));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, BYTE));
    s.i(StoreByte {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(Jump(write));
    s.i(Mark(done));
    s.i(LoadArg(A, 2));
    s.i(LoadSlot(B, DIGITS));
    s.i(Int(IntOp::Add));
    s.i(Return);
    s.finish("put_decimal", 4, 5)
}

pub fn routines() -> Vec<Function> {
    vec![put_decimal()]
}
