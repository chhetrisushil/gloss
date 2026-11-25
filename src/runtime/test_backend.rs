//! The routines a test build swaps in for the real ones: what `show` writes collects into
//! the shown bytes, and a run-time failure becomes the failure text, reported under the
//! test by `test_fail` instead of on standard error with status 2.

use super::asm::Asm;
use super::report::report_title;
use super::test_state::{
    ANSWERS, FAIL_BYTES, FAIL_LEN, FAILING, NEXT_ANSWER, SHOWN, SHOWN_LEN, get, set,
};
use crate::ir::Cond::{Eq, Ge, Gt, Le, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp, Prim, Shift};

/// Names of the real routines a test build replaces.
pub const REPLACED: [&str; 4] = ["write_all", "fail_begin", "fail_end", "ask"];

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

/// Marks the test as failing, then flushes what `show` left in the output buffer.
fn start_failing(s: &mut Asm) {
    s.i(Const(A, 1));
    set(s, FAILING, A, B);
    s.call("flush");
}

/// `fail_begin(line, code)`: `line <line>: failure <code>: ` into the failure text.
fn fail_begin() -> Function {
    let mut s = Asm::default();
    start_failing(&mut s);
    for (text, number) in [(super::LINE, 1), (super::FAILURE, 2)] {
        s.push_text(text);
        s.call("err_text");
        s.push_arg(number);
        s.call("err_int");
    }
    s.push_text(super::COLON);
    s.call("err_text");
    s.finish("fail_begin", 2, 0)
}

/// `fail_end()`: reports the failure. No newline is added to the failure text.
fn fail_end() -> Function {
    let mut s = Asm::default();
    s.call("test_fail");
    s.finish("fail_end", 0, 0)
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

/// `test_expect(line, source)`: fails with `line <line>: expect <source>`.
fn test_expect() -> Function {
    let mut s = Asm::default();
    // Flush first (R3), so what `show` left pending is shown before anything fails.
    s.call("flush");
    s.push_text(super::LINE);
    s.call("err_text");
    s.push_arg(1);
    s.call("err_int");
    s.push_text(super::COLON);
    s.call("err_text");
    s.push_text(super::EXPECT);
    s.call("err_text");
    s.push_arg(2);
    s.call("err_text");
    s.call("test_fail");
    s.finish("test_expect", 2, 0)
}

/// `test_input(text, line)`: puts `text` at the end of the answers. The first call makes the
/// list, `list_new(4, line)`, so running out of memory names the line of the `input`.
fn test_input() -> Function {
    let mut s = Asm::default();
    let have = s.label();
    get(&mut s, A, ANSWERS);
    s.i(Const(B, 0));
    s.jump_if(Ne, have);
    s.push_const(4);
    s.push_arg(2);
    s.call("list_new");
    set(&mut s, ANSWERS, A, B);
    s.i(Mark(have));
    get(&mut s, A, ANSWERS);
    s.i(Push(A));
    s.push_arg(1);
    s.push_arg(2);
    s.call("list_push");
    s.finish("test_input", 2, 0)
}

/// `test_ask_fail(line, given, asked)`: fails with `the test gave <given> answer[s] but line
/// <line> asked a <ordinal> question`; `asked` is the number of the question, 1 up. Past the
/// tenth, ` asked question number <asked>` stands in for the ordinal.
fn test_ask_fail() -> Function {
    use super::{ANSWER, ASKED_A, ASKED_NUMBER, BUT_LINE, FIRST_ORDINAL, GAVE, PLURAL_S, QUESTION};
    let mut s = Asm::default();
    let (one, number, done, end) = (s.label(), s.label(), s.label(), s.label());
    s.call("flush");
    s.push_text(GAVE);
    s.call("err_text");
    s.push_arg(2);
    s.call("err_int");
    s.push_text(ANSWER);
    s.call("err_text");
    s.i(LoadArg(A, 2));
    s.i(Const(B, 1));
    s.jump_if(Eq, one);
    s.push_text(PLURAL_S);
    s.call("err_text");
    s.i(Mark(one));
    s.push_text(BUT_LINE);
    s.call("err_text");
    s.push_arg(1);
    s.call("err_int");
    s.i(LoadArg(A, 3));
    s.i(Const(B, 10));
    s.jump_if(Gt, number);
    s.push_text(ASKED_A);
    s.call("err_text");
    for k in 1..=10 {
        let next = s.label();
        s.i(LoadArg(A, 3));
        s.i(Const(B, k));
        s.jump_if(Ne, next);
        s.push_text(FIRST_ORDINAL + k as u32 - 1);
        s.call("err_text");
        s.i(Jump(done));
        s.i(Mark(next));
    }
    s.i(Mark(done));
    s.push_text(QUESTION);
    s.call("err_text");
    s.i(Jump(end));
    s.i(Mark(number));
    s.push_text(ASKED_NUMBER);
    s.call("err_text");
    s.push_arg(3);
    s.call("err_int");
    s.i(Mark(end));
    s.call("test_fail");
    s.finish("test_ask_fail", 3, 0)
}

/// `ask(question, line)`: the next queued answer. It prints nothing (the question is not part
/// of `shown`) and never touches standard input; with no answer left the test fails with
/// `test_ask_fail`. Slots: 1 the answers, 2 how many there are, 3 the address of the answer.
fn ask() -> Function {
    let mut s = Asm::default();
    let (none, fail) = (s.label(), s.label());
    get(&mut s, A, ANSWERS);
    s.i(Const(B, 0));
    s.jump_if(Eq, none);
    s.i(StoreSlot(1, A));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(2, A));
    get(&mut s, A, NEXT_ANSWER);
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, fail);
    s.i(Shift(Shift::Left, 3));
    s.i(StoreSlot(3, A));
    s.i(LoadSlot(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 16,
    });
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(3, A));
    get(&mut s, A, NEXT_ANSWER);
    s.i(AddImm(A, 1));
    set(&mut s, NEXT_ANSWER, A, B);
    s.i(LoadSlot(A, 3));
    s.i(Return);
    s.i(Mark(none));
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    s.i(Mark(fail));
    s.push_arg(2);
    s.push_slot(2);
    get(&mut s, A, NEXT_ANSWER);
    s.i(AddImm(A, 1));
    s.i(Push(A));
    s.call("test_ask_fail");
    s.i(Return);
    s.finish("ask", 2, 3)
}

/// The test versions, appended after the real routines that are kept.
pub fn routines() -> Vec<Function> {
    vec![
        write_all(),
        fail_begin(),
        fail_end(),
        test_fail(),
        test_expect(),
        test_input(),
        test_ask_fail(),
        ask(),
    ]
}
