//! Lists: the header `[count, capacity, items]` and the block of 8-byte item slots it names.

use super::asm::Asm;
use crate::ir::Function;
use crate::ir::Inst::*;
use crate::ir::Reg::{A, B};
use crate::ir::Shift;

/// `list_new(room, line)`: the address of a header with count 0 and capacity `room`, whose
/// items are a fresh zeroed block of `room` slots. `room` is a small constant the compiler
/// chose, so `8 * room` cannot overflow. Slot 1: the header.
fn list_new() -> Function {
    let mut s = Asm::default();
    s.push_const(24);
    s.push_arg(2);
    s.call("alloc");
    s.i(StoreSlot(1, A));
    s.i(LoadArg(A, 1));
    s.i(Shift(Shift::Left, 3));
    s.i(Push(A));
    s.push_arg(2);
    s.call("alloc");
    s.i(Move { to: B, from: A });
    s.i(LoadSlot(A, 1));
    s.i(Store {
        base: A,
        offset: 16,
        from: B,
    });
    s.i(LoadArg(B, 1));
    s.i(Store {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(Return);
    s.finish("list_new", 2, 1)
}

pub fn routines() -> Vec<Function> {
    vec![list_new()]
}
