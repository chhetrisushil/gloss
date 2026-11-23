//! `Num*` programs, failure paths included: each runs one operation on `a` and `b`, then
//! writes `a` and `b` on success, or `b` and a marker on a failure jump (where `a` is
//! undefined), so a clobbered `b` shows too.

use super::{program, write_text, write_words};
use crate::ir::Reg::{A, B};
use crate::ir::{Inst, Label, Module, Prim};

/// Builds a `Num*` instruction from the `range` and `zero` labels.
type Op<'a> = &'a dyn Fn(Label, Label) -> Inst;

/// An operation's name, how to build it, and the operand pairs to try.
type Group<'a> = (&'a str, Op<'a>, &'a [(i64, i64)]);

/// One program for `op`, whose labels are `range`, then `zero`.
fn module(x: i64, y: i64, op: Op) -> Module {
    let mut m = Module::default();
    let (range_text, zero_text) = (m.text(b"R"), m.text(b"Z"));
    let (range, zero, done) = (Label(1), Label(2), Label(3));
    let mut body = vec![Inst::Const(A, x), Inst::Const(B, y), op(range, zero)];
    body.extend([Inst::Push(B), Inst::Push(A)]);
    body.extend(write_words(2));
    body.extend([Inst::Jump(done), Inst::Mark(range), Inst::Push(B)]);
    body.extend(write_words(1));
    body.extend(write_text(range_text, 1));
    body.extend([Inst::Jump(done), Inst::Mark(zero), Inst::Push(B)]);
    body.extend(write_words(1));
    body.extend(write_text(zero_text, 1));
    body.extend([Inst::Mark(done), Inst::Const(A, 0), Inst::Push(A)]);
    body.push(Inst::System(Prim::Exit));
    program(&mut m, body);
    m
}

/// Calls `each` with the name and module of every program.
pub fn programs(each: &mut dyn FnMut(&str, &Module)) {
    let (min, max) = (i64::MIN, i64::MAX);
    let add = |r: Label, _: Label| Inst::NumAdd { range: r };
    let sub = |r: Label, _: Label| Inst::NumSub { range: r };
    let neg = |r: Label, _: Label| Inst::NumNeg { range: r };
    let mul = |r: Label, _: Label| Inst::NumMul { range: r };
    let div = |range: Label, zero: Label| Inst::NumDiv { zero, range };
    let rem = |_: Label, zero: Label| Inst::NumRem { zero };
    let whole = |_: Label, _: Label| Inst::NumWhole;
    let ops: [Group; 7] = [
        (
            "add",
            &add,
            &[(max, 1), (min, -1), (5, -12), (max, 0), (max - 1, 1)],
        ),
        (
            "sub",
            &sub,
            &[(min, 1), (max, -1), (0, min), (5, 12), (-3, -3)],
        ),
        ("neg", &neg, &[(min, 77), (0, 77), (5, 77), (-max, 77)]),
        (
            "mul",
            &mul,
            &[(max, max), (123_456, 789_012), (-5, 5), (min, 10_000)],
        ),
        (
            "div",
            &div,
            &[(5, 0), (min, -1), (1, 3), (-1, 3), (7, -2), (max, 1)],
        ),
        (
            "rem",
            &rem,
            &[(5, 0), (min, -1), (-7, 3), (7, -3), (max, min)],
        ),
        (
            "whole",
            &whole,
            &[(-123_456, 5), (min, 5), (9999, 5), (-9999, 5), (max, 5)],
        ),
    ];
    for (name, op, pairs) in ops {
        for (x, y) in pairs {
            each(&format!("{name} {x} {y}"), &module(*x, *y, op));
        }
    }
}
