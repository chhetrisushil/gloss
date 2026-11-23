//! The allocator.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Le, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Global, Inst::*, IntOp, Prim};

/// The least the allocator maps at a time.
const PIECE: i64 = 64 << 20;

fn heap_init() -> Function {
    let mut s = Asm::default();
    s.i(Const(A, 0));
    s.gstore(Global::HeapNext, A, B);
    s.gstore(Global::HeapEnd, A, B);
    s.finish("heap_init", 0, 0)
}

/// `alloc(bytes, line)`: the address of `bytes` zeroed bytes (at least 8, rounded up to a
/// multiple of 8). Slots: 1 the rounded size, 2 the size of a new piece, 3 the result.
fn alloc() -> Function {
    let mut s = Asm::default();
    let (nonzero, fits, big, mapped) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(AddImm(A, 7));
    s.i(Const(B, -8));
    s.i(Int(IntOp::And));
    s.i(Const(B, 0));
    s.jump_if(Ne, nonzero);
    s.i(Const(A, 8));
    s.i(Mark(nonzero));
    s.i(StoreSlot(1, A));
    s.gload(A, Global::HeapNext);
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.gload(B, Global::HeapEnd);
    s.jump_if(Le, fits);
    // The block does not fit: map a piece of `max(PIECE, size)` and start allocating there.
    s.i(LoadSlot(A, 1));
    s.i(Const(B, PIECE));
    s.jump_if(Ge, big);
    s.i(Move { to: A, from: B });
    s.i(Mark(big));
    s.i(StoreSlot(2, A));
    s.i(Push(A));
    s.i(System(Prim::Map));
    s.i(Const(B, 0));
    s.jump_if(Ne, mapped);
    s.push_arg(2);
    s.call("fail_memory");
    s.i(Mark(mapped));
    s.gstore(Global::HeapNext, A, B);
    s.i(LoadSlot(B, 2));
    s.i(Int(IntOp::Add));
    s.gstore(Global::HeapEnd, A, B);
    s.i(Mark(fits));
    s.gload(A, Global::HeapNext);
    s.i(StoreSlot(3, A));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.gstore(Global::HeapNext, A, B);
    s.i(LoadSlot(A, 3));
    s.i(Return);
    s.finish("alloc", 2, 3)
}

fn fail_memory() -> Function {
    let mut s = Asm::default();
    s.push_arg(1);
    s.push_const(10);
    s.call("fail_begin");
    s.push_text(super::OUT_OF_MEMORY);
    s.call("err_text");
    s.call("fail_end");
    s.finish("fail_memory", 1, 0)
}

pub fn routines() -> Vec<Function> {
    vec![heap_init(), alloc(), fail_memory()]
}
