//! The program's own arguments: `--permissions` prints the permission list and exits.

use super::{PERMISSIONS_FLAG, asm::Asm};
use crate::ir::Cond::{Eq, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, Prim};

/// `args_check`: when the only argument is `--permissions`, byte for byte, writes the text
/// `permissions_text()` gives, flushes and exits 0; otherwise returns. The flag's text ends in
/// a NUL, so the compare also checks that the argument ends there. Slots: 1 the cursor in
/// the argument, 2 the cursor in the flag.
fn args_check() -> Function {
    let mut s = Asm::default();
    let (again, matched, done) = (s.label(), s.label(), s.label());
    s.gload(A, Global::EntrySp);
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 2));
    s.jump_if(Ne, done);
    s.gload(A, Global::EntrySp);
    s.i(Load {
        to: A,
        base: A,
        offset: 16,
    });
    s.i(StoreSlot(1, A));
    s.i(AddressOf(A, Data::Text(PERMISSIONS_FLAG)));
    s.i(AddImm(A, 8));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(LoadSlot(B, 2));
    s.i(LoadByte {
        to: B,
        base: B,
        offset: 0,
    });
    s.jump_if(Ne, done);
    s.i(Const(B, 0));
    s.jump_if(Eq, matched);
    for slot in [1, 2] {
        s.i(LoadSlot(A, slot));
        s.i(AddImm(A, 1));
        s.i(StoreSlot(slot, A));
    }
    s.i(Jump(again));
    s.i(Mark(matched));
    s.call("permissions_text");
    s.i(Push(A));
    s.call("out_text");
    s.call("flush");
    s.push_const(0);
    s.i(System(Prim::Exit));
    s.i(Mark(done));
    s.finish("args_check", 0, 2)
}

pub fn routines() -> Vec<Function> {
    vec![args_check()]
}
