//! Cases for frames, arguments, slots and calls, ported from the reviewer's 8a harness.

use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Function, Inst, IntOp, Label};
use Inst::*;

fn f(name: &str, params: u32, slots: u32, body: Vec<Inst>) -> Function {
    Function {
        name: name.to_string(),
        params,
        slots,
        body,
    }
}

pub fn l(n: u32) -> Label {
    Label(n)
}

/// Functions the cases call; they are not cases themselves.
pub fn helpers() -> Vec<Function> {
    Vec::new()
}

pub fn cases() -> Vec<Function> {
    vec![f(
        "sum_loop",
        0,
        2,
        vec![
            Enter(2),
            Const(A, 1),
            StoreSlot(2, A),
            Mark(l(1)),
            LoadSlot(A, 2),
            Const(B, 10),
            Compare,
            JumpIf(Cond::Gt, l(2)),
            LoadSlot(A, 1),
            LoadSlot(B, 2),
            Int(IntOp::Add),
            StoreSlot(1, A),
            LoadSlot(A, 2),
            AddImm(A, 1),
            StoreSlot(2, A),
            Jump(l(1)),
            Mark(l(2)),
            LoadSlot(A, 1),
            Return,
        ],
    )]
}
