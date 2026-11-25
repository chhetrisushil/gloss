//! The routines a trace build swaps in for the real ones. Later tasks add the event writers
//! and the replaced input and failure routines.
//!
//! Text enters the output buffer through `out_bytes`; a trace build's one decides, by the
//! trace state's `ESCAPING` word, whether it is JSON-escaped on the way (`json_bytes`, in
//! `json.rs`).

use super::asm::Asm;
use super::trace_state::{ESCAPING, get};
use crate::ir::Cond::Ne;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*};

/// Names of the real routines a trace build replaces.
pub const REPLACED: [&str; 1] = ["out_bytes"];

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
    vec![out_bytes()]
}
