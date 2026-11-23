//! Comparing text.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp};

// Slots of `text_equal`.
const LEN: u32 = 1;
const AT: u32 = 2;
const BYTE: u32 = 3;

/// The byte at `AT` of the text argument `n`, into `a` (clobbers `b`).
fn byte_of(s: &mut Asm, n: u32) {
    s.i(LoadArg(A, n));
    s.i(LoadSlot(B, AT));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
}

/// `text_equal(x, y)`: 1 when the two texts have the same bytes, else 0. It compares the
/// lengths, then the bytes in order; it never looks at addresses, so an interned text and a
/// built one with the same bytes are equal.
fn text_equal() -> Function {
    let mut s = Asm::default();
    let (differ, again, same) = (s.label(), s.label(), s.label());
    for n in [1, 2] {
        s.i(LoadArg(A, n));
        s.i(Load {
            to: A,
            base: A,
            offset: 0,
        });
        if n == 1 {
            s.i(StoreSlot(LEN, A));
        }
    }
    s.i(LoadSlot(B, LEN));
    s.jump_if(Ne, differ);
    s.i(Const(A, 0));
    s.i(StoreSlot(AT, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, LEN));
    s.jump_if(Ge, same);
    byte_of(&mut s, 1);
    s.i(StoreSlot(BYTE, A));
    byte_of(&mut s, 2);
    s.i(LoadSlot(B, BYTE));
    s.jump_if(Ne, differ);
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(AT, A));
    s.i(Jump(again));
    s.i(Mark(same));
    s.i(Const(A, 1));
    s.i(Return);
    s.i(Mark(differ));
    s.finish("text_equal", 2, 3)
}

pub fn routines() -> Vec<Function> {
    vec![text_equal()]
}
