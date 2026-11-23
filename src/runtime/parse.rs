//! Reading a number from text: `parse_number`.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Gt, Le, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Function, Global, Inst::*, IntOp, Label};

// Slots of `parse_number`.
const START: u32 = 1;
const END: u32 = 2;
const NEG: u32 = 3;
const MAG: u32 = 4;
const DIGIT: u32 = 5;
const FRAC: u32 = 6;
const LAST: u32 = 7;

/// The most negative magnitude that can still take another digit: `i64::MIN / 10`.
const LIMIT: i64 = i64::MIN / 10;

/// The byte of text argument 1 at the index in `slot`, into `a` (clobbers `b`).
fn byte_at(s: &mut Asm, slot: u32) {
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, slot));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
}

/// Jumps to `to` when `slot` is `cond` the constant `n`.
fn slot_is(s: &mut Asm, slot: u32, n: i64, cond: Cond, to: Label) {
    s.i(LoadSlot(A, slot));
    s.i(Const(B, n));
    s.jump_if(cond, to);
}

/// Adds 1 to `slot`.
fn bump(s: &mut Asm, slot: u32) {
    s.i(LoadSlot(A, slot));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(slot, A));
}

/// Jumps to `to` when `slot` is at or past the end of the text.
fn at_end(s: &mut Asm, slot: u32, to: Label) {
    s.i(LoadSlot(A, slot));
    s.i(LoadSlot(B, END));
    s.jump_if(Ge, to);
}

/// The byte at `START` minus `0`, into `DIGIT`; jumps to `other` when it is not a digit.
fn digit_or(s: &mut Asm, other: Label) {
    byte_at(s, START);
    s.i(AddImm(A, -i32::from(b'0')));
    s.i(StoreSlot(DIGIT, A));
    s.i(Const(B, 0));
    s.jump_if(Lt, other);
    slot_is(s, DIGIT, 9, Gt, other);
}

/// `MAG = MAG * 10 - DIGIT`, or a jump to `fail` when that goes below `i64::MIN`. The
/// magnitude is kept negative so that `-922337203685477.5808` needs no special case.
fn append_digit(s: &mut Asm, fail: Label) {
    let ok = s.label();
    slot_is(s, MAG, LIMIT, Lt, fail);
    slot_is(s, MAG, LIMIT, Gt, ok);
    slot_is(s, DIGIT, -(i64::MIN % 10), Gt, fail);
    s.i(Mark(ok));
    s.i(LoadSlot(A, MAG));
    s.i(Const(B, 10));
    s.i(Int(IntOp::Mul));
    s.i(LoadSlot(B, DIGIT));
    s.i(Int(IntOp::Sub));
    s.i(StoreSlot(MAG, A));
}

/// `parse_number(text)`: 0 when the text is a number, else 1. The number is left in the
/// first 8 bytes of `Global::NumBuf` (0 when the status is 1), valid until the next
/// `number_format` or `parse_number`: one result only comes back in `a`, and `b` does not
/// survive a call. The accepted form is `Number::parse_text`'s: spaces at the ends ignored,
/// an optional `-`, digits, and optionally `.` and 1 to 4 digits, within range.
fn parse_number() -> Function {
    let mut s = Asm::default();
    let [front, front_done, back, back_done, signed, whole, whole_end] = [(); 7].map(|_| s.label());
    let [frac, frac_end, pad, finish, negative, store, fail] = [(); 7].map(|_| s.label());
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(END, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(START, A));
    s.i(StoreSlot(NEG, A));
    s.i(StoreSlot(MAG, A));
    s.i(StoreSlot(FRAC, A));
    // Spaces at the front, then at the back.
    s.i(Mark(front));
    at_end(&mut s, START, front_done);
    byte_at(&mut s, START);
    s.i(Const(B, i64::from(b' ')));
    s.jump_if(Ne, front_done);
    bump(&mut s, START);
    s.i(Jump(front));
    s.i(Mark(front_done));
    s.i(Mark(back));
    s.i(LoadSlot(A, END));
    s.i(LoadSlot(B, START));
    s.jump_if(Le, back_done);
    s.i(LoadSlot(A, END));
    s.i(AddImm(A, -1));
    s.i(StoreSlot(LAST, A));
    byte_at(&mut s, LAST);
    s.i(Const(B, i64::from(b' ')));
    s.jump_if(Ne, back_done);
    s.i(LoadSlot(A, LAST));
    s.i(StoreSlot(END, A));
    s.i(Jump(back));
    s.i(Mark(back_done));
    // The sign.
    at_end(&mut s, START, signed);
    byte_at(&mut s, START);
    s.i(Const(B, i64::from(b'-')));
    s.jump_if(Ne, signed);
    s.i(Const(A, 1));
    s.i(StoreSlot(NEG, A));
    bump(&mut s, START);
    s.i(Mark(signed));
    // One or more whole digits; LAST counts them.
    s.i(Const(A, 0));
    s.i(StoreSlot(LAST, A));
    s.i(Mark(whole));
    at_end(&mut s, START, whole_end);
    digit_or(&mut s, whole_end);
    append_digit(&mut s, fail);
    bump(&mut s, LAST);
    bump(&mut s, START);
    s.i(Jump(whole));
    s.i(Mark(whole_end));
    slot_is(&mut s, LAST, 0, Eq, fail);
    at_end(&mut s, START, pad);
    byte_at(&mut s, START);
    s.i(Const(B, i64::from(b'.')));
    s.jump_if(Ne, fail);
    bump(&mut s, START);
    // One to four fraction digits.
    s.i(Mark(frac));
    at_end(&mut s, START, frac_end);
    digit_or(&mut s, fail);
    slot_is(&mut s, FRAC, 4, Ge, fail);
    append_digit(&mut s, fail);
    bump(&mut s, FRAC);
    bump(&mut s, START);
    s.i(Jump(frac));
    s.i(Mark(frac_end));
    slot_is(&mut s, FRAC, 0, Eq, fail);
    // Zeros for the places the text left out.
    s.i(Mark(pad));
    slot_is(&mut s, FRAC, 4, Ge, finish);
    s.i(Const(A, 0));
    s.i(StoreSlot(DIGIT, A));
    append_digit(&mut s, fail);
    bump(&mut s, FRAC);
    s.i(Jump(pad));
    s.i(Mark(finish));
    // A magnitude of -2^63 is only a number when the text had a minus sign.
    slot_is(&mut s, NEG, 0, Ne, negative);
    slot_is(&mut s, MAG, i64::MIN, Eq, fail);
    s.i(Const(A, 0));
    s.i(LoadSlot(B, MAG));
    s.i(Int(IntOp::Sub));
    s.i(Jump(store));
    s.i(Mark(negative));
    s.i(LoadSlot(A, MAG));
    s.i(Mark(store));
    s.gstore(Global::NumBuf, A, B);
    s.i(Const(A, 0));
    s.i(Return);
    s.i(Mark(fail));
    s.i(Const(A, 0));
    s.gstore(Global::NumBuf, A, B);
    s.i(Const(A, 1));
    s.i(Return);
    s.finish("parse_number", 1, 7)
}

pub fn routines() -> Vec<Function> {
    vec![parse_number()]
}
