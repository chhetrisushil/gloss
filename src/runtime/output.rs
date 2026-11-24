//! Writing to the output descriptors, with standard output buffered, and the failure
//! message pieces.

use super::asm::Asm;
use crate::ir::Cond::{Ge, Gt, Le, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Prim};

/// The size of `Global::OutBuf`.
const OUT_BUF: i64 = crate::ir::IO_BUF_BYTES as i64;

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

/// `file_write(fd, addr, len)`: writes every byte, looping on short writes; 0 when done, or
/// the first negative error (a write that gives 0 is `-5`, an I/O error). Unlike `write_all`
/// it never ends the program, so a save can name the failure. Slots: 1 the address, 2 what
/// is left.
fn file_write() -> Function {
    let mut s = Asm::default();
    let (again, advance, error, done) = (s.label(), s.label(), s.label(), s.label());
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
    s.jump_if(Gt, advance);
    s.jump_if(Lt, error);
    s.i(Const(A, -5));
    s.i(Mark(error));
    s.i(Return);
    s.i(Mark(advance));
    s.i(Move { to: B, from: A });
    s.i(LoadSlot(A, 1));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(1, A));
    s.i(LoadSlot(A, 2));
    s.i(Int(IntOp::Sub));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("file_write", 3, 2)
}

fn flush() -> Function {
    let mut s = Asm::default();
    s.push_const(1);
    s.i(AddressOf(A, Data::Global(Global::OutBuf)));
    s.i(Push(A));
    s.gload(A, Global::OutLen);
    s.i(Push(A));
    s.call("write_all");
    s.i(Const(A, 0));
    s.gstore(Global::OutLen, A, B);
    s.finish("flush", 0, 0)
}

/// `out_bytes(addr, len)`: appends to the buffer; a write that cannot fit even in an empty
/// buffer goes straight out after a flush.
fn out_bytes() -> Function {
    let mut s = Asm::default();
    let (copy, copying, copied) = (s.label(), s.label(), s.label());
    s.gload(A, Global::OutLen);
    s.i(Move { to: B, from: A });
    s.i(Const(A, OUT_BUF));
    s.i(Int(IntOp::Sub));
    s.i(LoadArg(B, 2));
    s.jump_if(Ge, copy);
    s.call("flush");
    s.i(LoadArg(A, 2));
    s.i(Const(B, OUT_BUF));
    s.jump_if(Le, copy);
    s.push_const(1);
    s.push_arg(1);
    s.push_arg(2);
    s.call("write_all");
    s.i(Return);
    // Copy byte by byte: slot 1 is the source, 2 the destination, 3 the end of the source.
    s.i(Mark(copy));
    s.i(LoadArg(A, 1));
    s.i(StoreSlot(1, A));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(3, A));
    s.i(AddressOf(A, Data::Global(Global::OutBuf)));
    s.gload(B, Global::OutLen);
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(2, A));
    s.i(Mark(copying));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 3));
    s.jump_if(Ge, copied);
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(LoadSlot(B, 2));
    s.i(StoreByte {
        base: B,
        offset: 0,
        from: A,
    });
    for slot in [1, 2] {
        s.i(LoadSlot(A, slot));
        s.i(AddImm(A, 1));
        s.i(StoreSlot(slot, A));
    }
    s.i(Jump(copying));
    s.i(Mark(copied));
    s.gload(A, Global::OutLen);
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.gstore(Global::OutLen, A, B);
    s.finish("out_bytes", 2, 3)
}

fn out_text() -> Function {
    let mut s = Asm::default();
    s.push_text_arg(1);
    s.call("out_bytes");
    s.finish("out_text", 1, 0)
}

fn err_text() -> Function {
    let mut s = Asm::default();
    s.push_const(2);
    s.push_text_arg(1);
    s.call("write_all");
    s.finish("err_text", 1, 0)
}

/// Writes the byte in `a` to standard error, from the stack.
fn emit_byte(s: &mut Asm) {
    s.i(Push(A));
    s.push_const(2);
    s.i(StackPointer(A));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.push_const(1);
    s.i(System(Prim::Write));
    s.i(Pop(B));
}

/// `err_int(n)`: any `n` in decimal, from the highest power of ten down, skipping leading
/// zeros. It works on `-|n|` so that the most negative number needs no special case.
/// Slots: 1 the rest, 2 the power of ten, 3 whether a digit was written, 4 the digit.
fn err_int() -> Function {
    let mut s = Asm::default();
    let (positive, store, again, print, next, done) = (
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
    );
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Ge, positive);
    s.i(Const(A, i64::from(b'-')));
    emit_byte(&mut s);
    s.i(LoadArg(A, 1));
    s.i(Jump(store));
    s.i(Mark(positive));
    s.i(Move { to: B, from: A });
    s.i(Const(A, 0));
    s.i(Int(IntOp::Sub));
    s.i(Mark(store));
    s.i(StoreSlot(1, A));
    s.i(Const(A, 1_000_000_000_000_000_000));
    s.i(StoreSlot(2, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(3, A));
    s.i(Mark(again));
    for op in [IntOp::Div, IntOp::Rem] {
        s.i(LoadSlot(A, 1));
        s.i(LoadSlot(B, 2));
        s.i(Int(op));
        s.i(StoreSlot(if op == IntOp::Div { 4 } else { 1 }, A));
    }
    s.i(Const(A, 0));
    s.i(LoadSlot(B, 4));
    s.i(Int(IntOp::Sub));
    s.i(StoreSlot(4, A));
    s.i(Const(B, 0));
    s.jump_if(Ne, print);
    s.i(LoadSlot(A, 3));
    s.jump_if(Ne, print);
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 1));
    s.jump_if(Ne, next);
    s.i(Mark(print));
    s.i(Const(A, 1));
    s.i(StoreSlot(3, A));
    s.i(LoadSlot(A, 4));
    s.i(AddImm(A, i32::from(b'0')));
    emit_byte(&mut s);
    s.i(Mark(next));
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 1));
    s.jump_if(Le, done);
    s.i(Const(B, 10));
    s.i(Int(IntOp::Div));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("err_int", 1, 4)
}

/// `fail_begin(line, code)`: flushes, then starts `line <line>: failure <code>: `.
fn fail_begin() -> Function {
    let mut s = Asm::default();
    s.call("flush");
    s.push_text(super::LINE);
    s.call("err_text");
    s.push_arg(1);
    s.call("err_int");
    s.push_text(super::FAILURE);
    s.call("err_text");
    s.push_arg(2);
    s.call("err_int");
    s.push_text(super::COLON);
    s.call("err_text");
    s.finish("fail_begin", 2, 0)
}

fn fail_end() -> Function {
    let mut s = Asm::default();
    s.push_text(super::NEWLINE);
    s.call("err_text");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.finish("fail_end", 0, 0)
}

pub fn routines() -> Vec<Function> {
    vec![
        write_all(),
        file_write(),
        flush(),
        out_bytes(),
        out_text(),
        err_text(),
        err_int(),
        fail_begin(),
        fail_end(),
    ]
}
