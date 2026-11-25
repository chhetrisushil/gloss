//! The routines a trace build swaps in for the real ones: `out_bytes`, and the failure
//! routines, which turn a run-time failure into one `fail` event on standard output. Later
//! tasks add the event writers and the replaced input routines.
//!
//! Text enters the output buffer through `out_bytes`; a trace build's one decides, by the
//! trace state's `ESCAPING` word, whether it is JSON-escaped on the way (`json_bytes`, in
//! `json.rs`).

use super::asm::Asm;
use super::trace_state::{ESCAPING, FAILING, OPEN, get, set};
use super::values::out_const;
use super::{FAIL_CLOSE, FAIL_EVENT, FAIL_LINE, FAIL_MESSAGE, NEWLINE};
use crate::ir::Cond::{Eq, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, Prim};

/// Names of the real routines a trace build replaces.
pub const REPLACED: [&str; 4] = ["out_bytes", "write_all", "fail_begin", "fail_end"];

/// Stores the constant `n` in the trace state word at `offset`.
fn put_word(s: &mut Asm, offset: i32, n: i64) {
    s.i(Const(A, n));
    set(s, offset, A, B);
}

/// `write_all(fd, addr, len)`: descriptor 2 while `FAILING` is the failure message, which goes
/// into the event through `out_bytes` (escaped); anything else takes the real loop.
fn write_all() -> Function {
    let mut s = Asm::default();
    let (real, failing) = (s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(Const(B, 2));
    s.jump_if(Ne, real);
    get(&mut s, A, FAILING);
    s.i(Const(B, 0));
    s.jump_if(Ne, failing);
    s.i(Mark(real));
    s.push_arg(1);
    s.push_arg(2);
    s.push_arg(3);
    s.call("write_loop");
    s.i(Return);
    s.i(Mark(failing));
    s.push_arg(2);
    s.push_arg(3);
    s.call("out_bytes");
    s.finish("write_all", 3, 0)
}

/// `fail_begin(line, code)`: closes an open `show` event (`"}` and a newline), then writes
/// `{"line":<line>,"event":"fail","failure":<code>,"message":"` and starts `FAILING`, with
/// the message escaped.
fn fail_begin() -> Function {
    let mut s = Asm::default();
    let closed = s.label();
    put_word(&mut s, ESCAPING, 0);
    get(&mut s, A, OPEN);
    s.i(Const(B, 0));
    s.jump_if(Eq, closed);
    out_const(&mut s, FAIL_CLOSE);
    put_word(&mut s, OPEN, 0);
    s.i(Mark(closed));
    out_const(&mut s, FAIL_LINE);
    s.push_arg(1);
    s.call("json_int");
    out_const(&mut s, FAIL_EVENT);
    s.push_arg(2);
    s.call("json_int");
    out_const(&mut s, FAIL_MESSAGE);
    put_word(&mut s, FAILING, 1);
    put_word(&mut s, ESCAPING, 1);
    s.finish("fail_begin", 2, 0)
}

/// `fail_end()`: ends the message (`"}` and a newline), flushes and exits with status 2.
/// With no `fail_begin` before it (`FAILING` is 0: the internal error of a feature that ends
/// without giving) it is the real one: a newline on descriptor 2 and exit 2, no event.
fn fail_end() -> Function {
    let mut s = Asm::default();
    let failing = s.label();
    get(&mut s, A, FAILING);
    s.i(Const(B, 0));
    s.jump_if(Ne, failing);
    s.push_text(NEWLINE);
    s.call("err_text");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.i(Mark(failing));
    put_word(&mut s, ESCAPING, 0);
    out_const(&mut s, FAIL_CLOSE);
    s.call("flush");
    s.push_const(2);
    s.i(System(Prim::Exit));
    s.finish("fail_end", 0, 0)
}

/// `out_bytes(addr, len)`: `buf_bytes` while not `ESCAPING`, else `json_bytes`.
fn out_bytes() -> Function {
    let mut s = Asm::default();
    let escape = s.label();
    get(&mut s, A, ESCAPING);
    s.i(Const(B, 0));
    s.jump_if(Ne, escape);
    s.push_arg(1);
    s.push_arg(2);
    s.call("buf_bytes");
    s.i(Return);
    s.i(Mark(escape));
    s.push_arg(1);
    s.push_arg(2);
    s.call("json_bytes");
    s.finish("out_bytes", 2, 0)
}

/// The routines a trace build adds.
pub fn routines() -> Vec<Function> {
    vec![out_bytes(), write_all(), fail_begin(), fail_end()]
}
