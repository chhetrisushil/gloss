use super::call;
use crate::ir::{Cond, Function, Inst, IntOp, Label, Module, Reg, Shift};
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

#[test]
fn a_loop_sums_one_to_ten() {
    let (top, done) = (Label(0), Label(1));
    let m = with_slots(
        2,
        vec![
            Const(A, 1),
            StoreSlot(1, A),
            Mark(top),
            LoadSlot(A, 1),
            Const(B, 10),
            Compare,
            JumpIf(Cond::Gt, done),
            LoadSlot(A, 2),
            LoadSlot(B, 1),
            Int(IntOp::Add),
            StoreSlot(2, A),
            LoadSlot(A, 1),
            AddImm(A, 1),
            StoreSlot(1, A),
            Jump(top),
            Mark(done),
            LoadSlot(A, 2),
            Return,
        ],
    );
    assert_eq!(call(&m, "f", &[]), 55);
}

fn two_functions(callee: Function, caller: Function) -> Module {
    Module {
        functions: vec![callee, caller],
        ..Module::default()
    }
}

#[test]
fn arguments_are_numbered_left_to_right() {
    let minus = function(
        "minus",
        2,
        0,
        vec![LoadArg(A, 1), LoadArg(B, 2), Int(IntOp::Sub), Return],
    );
    let outer = function(
        "outer",
        0,
        0,
        vec![
            Const(A, 10),
            Push(A),
            Const(A, 3),
            Push(A),
            Call("minus".to_string()),
            Return,
        ],
    );
    let m = two_functions(minus, outer);
    assert_eq!(call(&m, "outer", &[]), 7);
    assert_eq!(call(&m, "minus", &[20, 5]), 15);
}

#[test]
fn b_and_slots_survive_a_call() {
    let clobber = function(
        "clobber",
        0,
        1,
        vec![Const(B, 100), Const(A, 200), StoreSlot(1, A), Return],
    );
    let outer = function(
        "outer",
        0,
        1,
        vec![
            Const(A, 7),
            StoreSlot(1, A),
            Const(B, 9),
            Call("clobber".to_string()),
            LoadSlot(A, 1),
            Int(IntOp::Add),
            Return,
        ],
    );
    assert_eq!(call(&two_functions(clobber, outer), "outer", &[]), 16);
}

#[test]
#[should_panic(expected = "wrong number of arguments")]
fn calling_with_the_wrong_number_of_arguments_panics() {
    call(&single(vec![Return]), "f", &[1]);
}
