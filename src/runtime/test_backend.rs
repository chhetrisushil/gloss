//! The routines a test build swaps in for the real ones: what `show` writes collects into
//! the shown bytes, and a run-time failure becomes the failure text, reported under the
//! test by `test_fail` instead of on standard error with status 2.

use super::asm::Asm;
use super::report::report_title;
use super::test_state::{FAIL_BYTES, FAIL_LEN, FAILING, SHOWN, SHOWN_LEN, get, set};
use crate::ir::Cond::{Le, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, Prim};

/// Names of the real routines a test build replaces.
pub const REPLACED: [&str; 1] = ["write_all"];

/// `write_all(fd, addr, len)`: descriptor 1 is shown, 2 is the failure text, any other is
/// ignored. It never fails and never exits.
fn write_all() -> Function {
    let mut s = Asm::default();
    let done = s.label();
    for (fd, add) in [(1, "shown_add"), (2, "fail_add")] {
        let not_out = s.label();
        s.i(LoadArg(A, 1));
        s.i(Const(B, fd));
        s.jump_if(Ne, not_out);
        s.push_arg(2);
        s.push_arg(3);
        s.call(add);
        s.i(Jump(done));
        s.i(Mark(not_out));
    }
    s.i(Mark(done));
    s.finish("write_all", 3, 0)
}

/// `test_fail()`: `fail  <title>`, the failure text and, if anything was shown, the shown
/// bytes, then exit 1.
fn test_fail() -> Function {
    let mut s = Asm::default();
    let none = s.label();
    // Flush first, then fail: R3. What `show` left pending reaches the shown bytes.
    s.call("flush");
    s.i(Const(A, 1));
    set(&mut s, FAILING, A, B);
    s.push_text(super::FAIL);
    s.call("report_text");
    report_title(&mut s);
    s.i(AddressOf(A, Data::Global(Global::TestState)));
    s.i(AddImm(A, FAIL_BYTES as i32 * 8));
    s.i(Push(A));
    get(&mut s, A, FAIL_LEN);
    s.i(Push(A));
    s.push_const(6);
    s.call("report_lines");
    get(&mut s, A, SHOWN_LEN);
    s.i(Const(B, 0));
    s.jump_if(Le, none);
    s.push_text(super::SHOWN_HEAD);
    s.call("report_text");
    get(&mut s, A, SHOWN);
    s.i(AddImm(A, 8));
    s.i(Push(A));
    get(&mut s, A, SHOWN_LEN);
    s.i(Push(A));
    s.push_const(8);
    s.call("report_lines");
    s.i(Mark(none));
    s.push_const(1);
    s.i(System(Prim::Exit));
    s.finish("test_fail", 0, 0)
}

/// The test versions, appended after the real routines that are kept.
pub fn routines() -> Vec<Function> {
    vec![write_all(), test_fail()]
}
