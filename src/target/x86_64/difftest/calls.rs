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
    vec![
        // reads three args in order, with its own slots and pushes
        f(
            "args3",
            3,
            2,
            vec![
                Enter(2),
                Const(A, 9),
                Push(A),
                Push(A),
                LoadArg(A, 1),
                Const(B, 1_000_000),
                Int(IntOp::Mul),
                StoreSlot(1, A),
                LoadArg(A, 2),
                Const(B, 1000),
                Int(IntOp::Mul),
                StoreSlot(2, A),
                LoadArg(A, 3),
                LoadSlot(B, 1),
                Int(IntOp::Add),
                LoadSlot(B, 2),
                Int(IntOp::Add),
                Return,
            ],
        ),
        f(
            "two",
            2,
            0,
            vec![
                Enter(0),
                LoadArg(A, 1),
                Const(B, 1000),
                Int(IntOp::Mul),
                LoadArg(B, 2),
                Int(IntOp::Add),
                Return,
            ],
        ),
        f(
            "clobber",
            0,
            3,
            vec![
                Enter(3),
                Const(B, -99),
                Const(A, 77),
                StoreSlot(1, A),
                StoreSlot(2, A),
                StoreSlot(3, A),
                Return,
            ],
        ),
        f(
            "dirty",
            0,
            6,
            vec![
                Enter(6),
                Const(A, 0x55),
                StoreSlot(1, A),
                StoreSlot(2, A),
                StoreSlot(3, A),
                StoreSlot(4, A),
                StoreSlot(5, A),
                StoreSlot(6, A),
                Return,
            ],
        ),
        f(
            "fresh",
            0,
            6,
            vec![
                Enter(6),
                LoadSlot(A, 1),
                LoadSlot(B, 2),
                Int(IntOp::Or),
                LoadSlot(B, 3),
                Int(IntOp::Or),
                LoadSlot(B, 4),
                Int(IntOp::Or),
                LoadSlot(B, 5),
                Int(IntOp::Or),
                LoadSlot(B, 6),
                Int(IntOp::Or),
                Return,
            ],
        ),
        f(
            "f3",
            1,
            0,
            vec![Enter(0), LoadArg(A, 1), AddImm(A, 1), Return],
        ),
        f(
            "f2",
            1,
            1,
            vec![
                Enter(1),
                LoadArg(A, 1),
                StoreSlot(1, A),
                Const(B, 1000),
                LoadSlot(A, 1),
                AddImm(A, 10),
                Push(A),
                Call("f3".into()),
                Int(IntOp::Mul),
                LoadSlot(B, 1),
                Int(IntOp::Add),
                Return,
            ],
        ),
        f(
            "f1",
            1,
            1,
            vec![
                Enter(1),
                LoadArg(A, 1),
                StoreSlot(1, A),
                AddImm(A, 1),
                Push(A),
                Call("f2".into()),
                LoadSlot(B, 1),
                Int(IntOp::Sub),
                Return,
            ],
        ),
    ]
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
