//! Writing to the output descriptors.

use super::asm::Asm;
use crate::ir::Cond::Le;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Prim};

/// `write_all(fd, addr, len)`: writes every byte, looping on short writes; a result that is
/// not positive ends the program with status 2.
fn write_all() -> Function {
    let mut s = Asm::default();
    let (again, fail, done) = (s.label(), s.label(), s.label());
    s.i(LoadArg(A, 2));
    s.i(StoreSlot(1, A));
    s.i(LoadArg(A, 3));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Le, done);
    s.push_arg(1);
    s.push_slot(1);
    s.push_slot(2);
    s.i(System(Prim::Write));
    s.i(Const(B, 0));
    s.jump_if(Le, fail);
    s.i(Move { to: B, from: A });
    s.i(LoadSlot(A, 1));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(1, A));
    s.i(LoadSlot(A, 2));
    s.i(Int(IntOp::Sub));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(fail));
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.i(Mark(done));
    s.finish("write_all", 3, 2)
}

pub fn routines() -> Vec<Function> {
    vec![write_all()]
}
