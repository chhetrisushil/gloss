//! Cases for memory, comparisons, integer operations and shifts, ported from the
//! reviewer's 8a harness.

use super::calls::l;
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Function, Inst, IntOp, Shift};
use Inst::*;

fn f(name: &str, slots: u32, body: Vec<Inst>) -> Function {
    Function {
        name: name.to_string(),
        params: 0,
        slots,
        body,
    }
}

/// Bitmask over all six conds, via `SetIf` or `JumpIf`.
fn conds(name: &str, x: i64, y: i64, jump: bool) -> Function {
    let all = [Cond::Eq, Cond::Ne, Cond::Lt, Cond::Le, Cond::Gt, Cond::Ge];
    let mut b = vec![Enter(1)];
    for (i, c) in all.iter().enumerate() {
        let i = i as u32;
        b.extend([Const(A, x), Const(B, y), Compare, Line(7)]);
        if jump {
            b.extend([
                JumpIf(*c, l(10 + 2 * i)),
                Const(A, 0),
                Jump(l(11 + 2 * i)),
                Mark(l(10 + 2 * i)),
                Const(A, 1),
                Mark(l(11 + 2 * i)),
            ]);
        } else {
            b.push(SetIf(*c));
        }
        // slot1 = slot1*2 + a
        b.extend([
            Move { to: B, from: A },
            LoadSlot(A, 1),
            Shift(Shift::Left, 1),
            Int(IntOp::Add),
            StoreSlot(1, A),
        ]);
    }
    b.extend([LoadSlot(A, 1), Return]);
    f(name, 1, b)
}

fn binop(name: &str, x: i64, y: i64, op: IntOp) -> Function {
    f(
        name,
        0,
        vec![Enter(0), Const(A, x), Const(B, y), Int(op), Return],
    )
}

fn sh(name: &str, x: i64, s: Shift, n: u8) -> Function {
    f(name, 0, vec![Enter(0), Const(A, x), Shift(s, n), Return])
}

pub fn cases() -> Vec<Function> {
    let mut c = Vec::new();
    let pairs = [
        (-5, -3),
        (-3, -5),
        (-4, -4),
        (-1, 1),
        (1, -1),
        (i64::MIN, i64::MAX),
        (i64::MAX, i64::MIN),
        (0, 0),
    ];
    for (i, (x, y)) in pairs.iter().enumerate() {
        c.push(conds(&format!("setif_{i}"), *x, *y, false));
        c.push(conds(&format!("jumpif_{i}"), *x, *y, true));
    }
    let divs = [
        (-7, 2),
        (7, 2),
        (-7, 3),
        (i64::MIN, 7),
        (i64::MIN, 1),
        (-1, 5),
        (100, 7),
        (i64::MAX, 2),
        (-6, 3),
    ];
    for (i, (x, y)) in divs.iter().enumerate() {
        c.push(binop(&format!("div_{i}"), *x, *y, IntOp::Div));
        c.push(binop(&format!("rem_{i}"), *x, *y, IntOp::Rem));
    }
    let ints = [
        (i64::MAX, 1, IntOp::Add),
        (i64::MIN, 1, IntOp::Sub),
        (i64::MAX, 3, IntOp::Mul),
        (-7, -9, IntOp::Mul),
        (-8, 0xF0, IntOp::And),
        (-16, 3, IntOp::Or),
        (5, -12, IntOp::Add),
    ];
    for (i, (x, y, op)) in ints.iter().enumerate() {
        c.push(binop(&format!("int_{i}"), *x, *y, *op));
    }
    let shifts = [
        (-3, Shift::Left, 4),
        (1, Shift::Left, 63),
        (-100, Shift::Right, 3),
        (-1, Shift::RightLogical, 60),
        (-8, Shift::RightLogical, 1),
        (-8, Shift::Right, 1),
        (i64::MIN, Shift::Right, 63),
        (i64::MIN, Shift::RightLogical, 63),
        (12345, Shift::Left, 0),
        (-12345, Shift::RightLogical, 0),
    ];
    for (i, (x, s, n)) in shifts.iter().enumerate() {
        c.push(sh(&format!("shift_{i}"), *x, *s, *n));
    }
    c
}
