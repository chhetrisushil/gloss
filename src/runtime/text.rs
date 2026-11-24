//! Comparing text.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Le, Ne};
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

/// `grow_text(old, len, room, line)`: a new text with room for `2 * room` bytes whose first
/// `len` bytes are those of `old`; its length word is left for the caller to set. Callers keep
/// `len <= 2 * room`, the capacity of the new text (`len <= room` for a growing builder; the
/// exact-size copy of a cell in `csv_read_cell` passes `room = (len + 1) / 2` and relies on
/// the wider bound). It allocates `2 * room + 8` bytes, so a `room` above 2^60 asks `alloc` for -1,
/// which is failure 10 like any size it refuses. Slots: 1 the new text, 2 how many bytes
/// are copied.
fn grow_text() -> Function {
    let mut s = Asm::default();
    let (small, asked, again, copied) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 3));
    s.i(Const(B, 1 << 60));
    s.jump_if(Le, small);
    s.i(Const(A, -1));
    s.i(Jump(asked));
    s.i(Mark(small));
    s.i(Move { to: B, from: A });
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 8));
    s.i(Mark(asked));
    s.i(Push(A));
    s.push_arg(4);
    s.call("alloc");
    s.i(StoreSlot(1, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(LoadArg(B, 2));
    s.jump_if(Ge, copied);
    s.i(LoadArg(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Push(A));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.i(Int(IntOp::Add));
    s.i(Pop(B));
    s.i(StoreByte {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(copied));
    s.i(LoadSlot(A, 1));
    s.i(Return);
    s.finish("grow_text", 4, 2)
}

pub fn routines() -> Vec<Function> {
    vec![text_equal(), grow_text()]
}
