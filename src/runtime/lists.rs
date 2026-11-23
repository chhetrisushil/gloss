//! Lists: the header `[count, capacity, items]` and the block of 8-byte item slots it names.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Le, Lt};
use crate::ir::Function;
use crate::ir::Inst::*;
use crate::ir::Reg::{A, B};
use crate::ir::{IntOp, Shift};

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

/// `list_push(list, value, line)`: puts `value` after the last item. A full list first moves
/// its items to a block of twice the room (the header stays put, so a loop variable keeps
/// pointing at the right item); a room above 2^58 asks `alloc` for -1, which it refuses as
/// out of memory, so doubling never overflows. Slots: 1 the new items, 2 how many are copied.
fn list_push() -> Function {
    let mut s = Asm::default();
    let (room, small, asked, again, copied) =
        (s.label(), s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: B,
        base: B,
        offset: 8,
    });
    s.jump_if(Lt, room);
    s.i(Move { to: A, from: B });
    s.i(Const(B, 1 << 58));
    s.jump_if(Le, small);
    s.i(Const(A, -1));
    s.i(Jump(asked));
    s.i(Mark(small));
    s.i(Shift(Shift::Left, 4));
    s.i(Mark(asked));
    s.i(Push(A));
    s.push_arg(3);
    s.call("alloc");
    s.i(StoreSlot(1, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: B,
        base: B,
        offset: 0,
    });
    s.jump_if(Ge, copied);
    s.i(Shift(Shift::Left, 3));
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: B,
        base: B,
        offset: 16,
    });
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Move { to: B, from: A });
    s.i(LoadSlot(A, 2));
    s.i(Shift(Shift::Left, 3));
    s.i(Push(B));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(Pop(B));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(copied));
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Shift(Shift::Left, 1));
    s.i(Move { to: B, from: A });
    s.i(LoadArg(A, 1));
    s.i(Store {
        base: A,
        offset: 8,
        from: B,
    });
    s.i(LoadSlot(B, 1));
    s.i(Store {
        base: A,
        offset: 16,
        from: B,
    });
    s.i(Mark(room));
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Shift(Shift::Left, 3));
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: B,
        base: B,
        offset: 16,
    });
    s.i(Int(IntOp::Add));
    s.i(LoadArg(B, 2));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: B,
        base: A,
        offset: 0,
    });
    s.i(AddImm(B, 1));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.finish("list_push", 3, 2)
}

pub fn routines() -> Vec<Function> {
    vec![list_new(), list_push()]
}
