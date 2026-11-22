use super::call;
use crate::ir::{Cond, Function, Inst, IntOp, Module, Reg, Shift};
use Inst::*;
use Reg::{A, B};

fn function(name: &str, params: u32, slots: u32, body: Vec<Inst>) -> Function {
    let mut all = vec![Enter(slots)];
    all.extend(body);
    Function {
        name: name.to_string(),
        params,
        slots,
        body: all,
    }
}

fn with_slots(slots: u32, body: Vec<Inst>) -> Module {
    Module {
        functions: vec![function("f", 0, slots, body)],
        ..Module::default()
    }
}

fn single(body: Vec<Inst>) -> Module {
    with_slots(0, body)
}

#[test]
fn a_function_returns_a() {
    let m = single(vec![Line(1), Const(A, 42), Return]);
    assert_eq!(call(&m, "f", &[]), 42);
}

#[test]
fn slots_start_zeroed_and_are_numbered_from_1() {
    let m = with_slots(
        2,
        vec![
            LoadSlot(B, 2),
            Const(A, 5),
            StoreSlot(1, A),
            Const(A, 9),
            StoreSlot(2, A),
            LoadSlot(A, 1),
            LoadSlot(B, 2),
            Int(IntOp::Sub),
            Return,
        ],
    );
    assert_eq!(call(&m, "f", &[]), -4);
}

#[test]
fn push_and_pop_are_last_in_first_out() {
    let m = single(vec![
        Const(A, 1),
        Push(A),
        Const(A, 2),
        Push(A),
        Pop(B),
        Pop(A),
        Return,
    ]);
    assert_eq!(call(&m, "f", &[]), 1);
}

#[test]
fn a_logical_right_shift_fills_with_zeros() {
    let m = single(vec![Const(A, -2), Shift(Shift::RightLogical, 1), Return]);
    assert_eq!(call(&m, "f", &[]), i64::MAX);
}

#[test]
fn arithmetic_shifts_keep_the_sign_and_ints_wrap() {
    let right = single(vec![Const(A, -8), Shift(Shift::Right, 1), Return]);
    assert_eq!(call(&right, "f", &[]), -4);
    let left = single(vec![Const(A, 3), Shift(Shift::Left, 62), Return]);
    assert_eq!(call(&left, "f", &[]), 3i64.wrapping_shl(62));
    let wrap = single(vec![
        Const(A, i64::MAX),
        Const(B, 1),
        Int(IntOp::Add),
        Return,
    ]);
    assert_eq!(call(&wrap, "f", &[]), i64::MIN);
}

#[test]
fn division_truncates_and_the_remainder_keeps_the_dividend_sign() {
    let div = single(vec![Const(A, -7), Const(B, 2), Int(IntOp::Div), Return]);
    assert_eq!(call(&div, "f", &[]), -3);
    let rem = single(vec![Const(A, -7), Const(B, 2), Int(IntOp::Rem), Return]);
    assert_eq!(call(&rem, "f", &[]), -1);
}

#[test]
fn set_if_reads_the_last_compare_and_move_copies() {
    let m = single(vec![
        Const(A, 4),
        Const(B, 5),
        Compare,
        SetIf(Cond::Lt),
        Move { to: B, from: A },
        AddImm(B, 4),
        Int(IntOp::Mul),
        Return,
    ]);
    assert_eq!(call(&m, "f", &[]), 5);
}
