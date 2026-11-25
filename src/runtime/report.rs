//! The report of a `gloss test` run. These routines write straight to descriptor 1 with
//! `Prim::Write`, never through `write_all`, `flush` or the output buffer, which belong to
//! the program under test.

use super::asm::Asm;
use super::test_state::TITLE;
use crate::ir::Cond::{Eq, Ge, Le};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Prim};

/// `report_bytes(addr, len)`: writes every byte to descriptor 1, looping on short writes; a
/// result that is not positive ends the process with status 2. Slots: 1 the address, 2 what
/// is left.
fn report_bytes() -> Function {
    let mut s = Asm::default();
    let (again, fail, done) = (s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(StoreSlot(1, A));
    s.i(LoadArg(A, 2));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Le, done);
    s.push_const(1);
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
    s.finish("report_bytes", 2, 2)
}

/// `report_text(text)`: the bytes of a text value.
fn report_text() -> Function {
    let mut s = Asm::default();
    s.push_text_arg(1);
    // push_text_arg pushes the address, then the length: the order report_bytes takes.
    s.call("report_bytes");
    s.finish("report_text", 1, 0)
}

/// `report_lines(addr, len, indent)`: each line of the bytes, indented by `indent` spaces
/// (0 to 8) and ended by a newline. A line ends at a newline; a last fragment without one
/// counts when it is not empty. Slots: 1 the line's start, 2 the bytes left, 3 its length.
fn report_lines() -> Function {
    let mut s = Asm::default();
    let (next, scan, scanned, done) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(StoreSlot(1, A));
    s.i(LoadArg(A, 2));
    s.i(StoreSlot(2, A));
    s.i(Mark(next));
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Le, done);
    s.i(AddressOf(A, Data::Text(super::INDENT)));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.push_arg(3);
    s.call("report_bytes");
    s.i(Const(A, 0));
    s.i(StoreSlot(3, A));
    s.i(Mark(scan));
    s.i(LoadSlot(A, 3));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, scanned);
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, i64::from(b'\n')));
    s.jump_if(Eq, scanned);
    s.i(LoadSlot(A, 3));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(3, A));
    s.i(Jump(scan));
    s.i(Mark(scanned));
    s.push_slot(1);
    s.push_slot(3);
    s.call("report_bytes");
    s.i(AddressOf(A, Data::Text(super::NEWLINE)));
    s.i(Push(A));
    s.call("report_text");
    // Step over the line and its newline; past the end the count goes negative and stops.
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(LoadSlot(A, 2));
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Sub));
    s.i(AddImm(A, -1));
    s.i(StoreSlot(2, A));
    s.i(Jump(next));
    s.i(Mark(done));
    s.finish("report_lines", 3, 3)
}

/// `test_begin(title)`: remembers the title text of the test being run, and allocates the
/// shown buffer so that a test that shows anything always has one, even once it is failing.
fn test_begin() -> Function {
    let mut s = Asm::default();
    s.i(LoadArg(A, 1));
    s.i(AddressOf(B, Data::Global(Global::TestState)));
    s.i(Store {
        base: B,
        offset: TITLE as i32 * 8,
        from: A,
    });
    s.call("shown_init");
    s.finish("test_begin", 1, 0)
}

/// Reports the title of the test being run.
fn report_title(s: &mut Asm) {
    s.i(AddressOf(A, Data::Global(Global::TestState)));
    s.i(Load {
        to: A,
        base: A,
        offset: TITLE as i32 * 8,
    });
    s.i(Push(A));
    s.call("report_text");
    s.push_text(super::NEWLINE);
    s.call("report_text");
}

/// Reports `n` (a number argument slot value times 10000) as a text.
fn report_number(s: &mut Asm, arg: u32) {
    s.i(LoadArg(A, arg));
    s.i(Const(B, 10000));
    s.i(Int(IntOp::Mul));
    s.i(Push(A));
    s.push_const(0);
    s.call("number_text");
    s.i(Push(A));
    s.call("report_text");
}

/// `test_pass()`: `pass  <title>`, then exit 0.
fn test_pass() -> Function {
    let mut s = Asm::default();
    s.push_text(super::PASS);
    s.call("report_text");
    report_title(&mut s);
    s.push_const(0);
    s.i(System(Prim::Exit));
    s.finish("test_pass", 0, 0)
}

/// `test_lost(title, status)`: the fail block of a test that ended without a result.
fn test_lost() -> Function {
    let mut s = Asm::default();
    s.push_text(super::FAIL);
    s.call("report_text");
    s.push_arg(1);
    s.call("report_text");
    s.push_text(super::NEWLINE);
    s.call("report_text");
    s.i(AddressOf(A, Data::Text(super::INDENT)));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.push_const(6);
    s.call("report_bytes");
    s.push_text(super::LOST);
    s.call("report_text");
    report_number(&mut s, 2);
    s.push_text(super::CLOSE);
    s.call("report_text");
    s.push_text(super::NEWLINE);
    s.call("report_text");
    s.finish("test_lost", 2, 0)
}

/// `test_summary(passed, failed)`: a blank line, then `N passed, M failed`.
fn test_summary() -> Function {
    let mut s = Asm::default();
    s.push_text(super::NEWLINE);
    s.call("report_text");
    report_number(&mut s, 1);
    s.push_text(super::PASSED);
    s.call("report_text");
    report_number(&mut s, 2);
    s.push_text(super::FAILED);
    s.call("report_text");
    s.finish("test_summary", 2, 0)
}

pub fn routines() -> Vec<Function> {
    vec![
        report_bytes(),
        report_text(),
        report_lines(),
        test_begin(),
        test_pass(),
        test_lost(),
        test_summary(),
    ]
}
