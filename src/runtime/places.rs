//! Converting a number to text with a fixed number of decimal places.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Le, Lt};
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

// Slots of `number_places`.
const NEG: u32 = 1;
const UNIT: u32 = 2;
const POWER: u32 = 3;
const BUF: u32 = 4;
const LEN: u32 = 5;
const ROUNDED: u32 = 6;
const TIMES: u32 = 7;

/// `slot = 10 ^ times`, where `times` is in `A`.
fn power_of_ten(s: &mut Asm, slot: u32) {
    let (again, done) = (s.label(), s.label());
    s.i(StoreSlot(TIMES, A));
    s.i(Const(A, 1));
    s.i(StoreSlot(slot, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, TIMES));
    s.i(Const(B, 0));
    s.jump_if(Le, done);
    s.i(AddImm(A, -1));
    s.i(StoreSlot(TIMES, A));
    s.i(LoadSlot(A, slot));
    s.i(Const(B, 10));
    s.i(Int(IntOp::Mul));
    s.i(StoreSlot(slot, A));
    s.i(Jump(again));
    s.i(Mark(done));
}

/// `a = -a`.
fn negate(s: &mut Asm) {
    s.i(Move { to: B, from: A });
    s.i(Const(A, 0));
    s.i(Int(IntOp::Sub));
}

/// Appends the byte `byte` to the text in `BUF`.
fn put_byte(s: &mut Asm, byte: u8) {
    s.i(LoadSlot(A, BUF));
    s.i(LoadSlot(B, LEN));
    s.i(Int(IntOp::Add));
    s.i(Const(B, i64::from(byte)));
    s.i(StoreByte {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(LoadSlot(A, LEN));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(LEN, A));
}

/// Calls `put_decimal(buf, len, <A>, width)` and keeps the new length; `width` is
/// the argument `places` (1) or 1.
fn put_number(s: &mut Asm, width: Option<u32>) {
    s.i(StoreSlot(TIMES, A));
    s.push_slot(BUF);
    s.push_slot(LEN);
    s.push_slot(TIMES);
    match width {
        Some(arg) => s.push_arg(arg),
        None => s.push_const(1),
    }
    s.call("put_decimal");
    s.i(StoreSlot(LEN, A));
}

/// `number_places(n, places, line)`: a fresh text with `n` rounded half away from zero to
/// `places` (0 to 4) decimal places, with every place written: `Number::with_places`.
/// It works on `-|n|` so that the most negative number needs no special case: the rounded
/// value (kept negative, as it may not fit otherwise) in units of the last place is the
/// quotient, one more when the remainder is half of a unit or more. The sign shows only when the rounded value is not zero.
fn number_places() -> Function {
    let mut s = Asm::default();
    let (kept, down, no_sign, finish) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Lt, kept);
    s.i(Move { to: B, from: A });
    s.i(Const(A, 0));
    s.i(Int(IntOp::Sub));
    s.i(Mark(kept));
    s.i(StoreSlot(NEG, A));
    s.i(Const(A, 4));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Sub));
    power_of_ten(&mut s, UNIT);
    s.i(LoadArg(A, 2));
    power_of_ten(&mut s, POWER);
    s.i(LoadSlot(A, NEG));
    s.i(LoadSlot(B, UNIT));
    s.i(Int(IntOp::Div));
    s.i(StoreSlot(ROUNDED, A));
    s.i(LoadSlot(A, NEG));
    s.i(Int(IntOp::Rem));
    // Twice the size of the remainder against one unit.
    s.i(Move { to: B, from: A });
    s.i(Const(A, 0));
    s.i(Int(IntOp::Sub));
    s.i(Move { to: B, from: A });
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, UNIT));
    s.jump_if(Lt, down);
    s.i(LoadSlot(A, ROUNDED));
    s.i(AddImm(A, -1));
    s.i(StoreSlot(ROUNDED, A));
    s.i(Mark(down));
    s.push_const(32);
    s.push_arg(3);
    s.call("alloc");
    s.i(StoreSlot(BUF, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(LEN, A));
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Ge, no_sign);
    s.i(LoadSlot(A, ROUNDED));
    s.jump_if(Eq, no_sign);
    put_byte(&mut s, b'-');
    s.i(Mark(no_sign));
    s.i(LoadSlot(A, ROUNDED));
    s.i(LoadSlot(B, POWER));
    s.i(Int(IntOp::Div));
    negate(&mut s);
    put_number(&mut s, None);
    s.i(LoadArg(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Eq, finish);
    put_byte(&mut s, b'.');
    s.i(LoadSlot(A, ROUNDED));
    s.i(LoadSlot(B, POWER));
    s.i(Int(IntOp::Rem));
    negate(&mut s);
    put_number(&mut s, Some(2));
    s.i(Mark(finish));
    s.i(LoadSlot(A, BUF));
    s.i(LoadSlot(B, LEN));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(LoadSlot(A, BUF));
    s.i(Return);
    s.finish("number_places", 3, 7)
}

pub fn routines() -> Vec<Function> {
    vec![put_decimal(), number_places()]
}
