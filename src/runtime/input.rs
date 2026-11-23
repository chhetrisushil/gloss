//! Reading standard input through `in_buf`.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Lt};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, Prim};

/// The size of `Global::InBuf`.
const IN_BUF: i64 = crate::ir::IO_BUF_BYTES as i64;

/// `in_fill()`: 1 when `in_buf` has an unread byte at `in_pos`, else 0 at end of input. When
/// `in_pos` has reached `in_len` it reads up to `in_buf`'s size afresh; it never keeps or
/// looks at more than `Read` gave. A `Read` that gives less than 0 ends the program with
/// status 2 and no message, after flushing, as a failed write does (M2-R32).
fn in_fill() -> Function {
    let mut s = Asm::default();
    let (ready, eof, error) = (s.label(), s.label(), s.label());
    s.gload(A, Global::InPos);
    s.gload(B, Global::InLen);
    s.jump_if(Lt, ready);
    s.push_const(0);
    s.i(AddressOf(A, Data::Global(Global::InBuf)));
    s.i(Push(A));
    s.push_const(IN_BUF);
    s.i(System(Prim::Read));
    s.i(Const(B, 0));
    s.jump_if(Lt, error);
    s.jump_if(Eq, eof);
    s.gstore(Global::InLen, A, B);
    s.i(Const(A, 0));
    s.gstore(Global::InPos, A, B);
    s.i(Mark(ready));
    s.i(Const(A, 1));
    s.i(Return);
    s.i(Mark(eof));
    s.i(Const(A, 0));
    s.i(Return);
    s.i(Mark(error));
    s.call("flush");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.finish("in_fill", 0, 0)
}

pub fn routines() -> Vec<Function> {
    vec![in_fill()]
}
