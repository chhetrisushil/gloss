use super::{call, run};
use crate::ir::{Cond, Data, Function, Global, Inst, IntOp, Label, Module, Reg, Shift};
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

#[test]
fn words_and_bytes_round_trip_through_a_global() {
    let out = Data::Global(Global::OutBuf);
    let word = single(vec![
        AddressOf(B, out),
        Const(A, -0x1122_3344_5566_7788),
        Store {
            base: B,
            offset: 8,
            from: A,
        },
        Const(A, 0),
        Load {
            to: A,
            base: B,
            offset: 8,
        },
        Return,
    ]);
    assert_eq!(call(&word, "f", &[]), -0x1122_3344_5566_7788);
    let byte = single(vec![
        AddressOf(B, out),
        Const(A, 0x1ff),
        StoreByte {
            base: B,
            offset: 3,
            from: A,
        },
        Load {
            to: A,
            base: B,
            offset: 0,
        },
        Return,
    ]);
    assert_eq!(call(&byte, "f", &[]), 0xff << 24);
    let back = single(vec![
        AddressOf(B, out),
        Const(A, -1),
        StoreByte {
            base: B,
            offset: 0,
            from: A,
        },
        LoadByte {
            to: A,
            base: B,
            offset: 0,
        },
        Return,
    ]);
    assert_eq!(call(&back, "f", &[]), 255);
}

#[test]
fn a_text_address_leads_to_its_length_and_then_its_bytes() {
    let mut m = single(vec![
        AddressOf(B, Data::Text(0)),
        LoadByte {
            to: A,
            base: B,
            offset: 8,
        },
        Return,
    ]);
    m.text(b"xyz");
    assert_eq!(call(&m, "f", &[]), i64::from(b'x'));
}

#[test]
fn a_call_lowers_the_stack_pointer_by_the_arguments_and_the_frame() {
    let callee = function(
        "callee",
        1,
        1,
        vec![StackPointer(A), LoadArg(B, 1), Int(IntOp::Sub), Return],
    );
    let caller = function(
        "caller",
        0,
        0,
        vec![StackPointer(A), Push(A), Call("callee".to_string()), Return],
    );
    let m = two_functions(callee, caller);
    let drop = call(&m, "caller", &[]);
    assert!(
        -drop >= 8 * (1 + 1 + 2),
        "the stack pointer fell by only {}",
        -drop
    );
}

#[test]
fn stack_check_jumps_when_the_frame_would_not_fit() {
    let fail = Label(0);
    let limit = i64::try_from(0x7fff_0000_0000u64 - 100).unwrap();
    let with_bytes = |bytes| {
        single(vec![
            AddressOf(B, Data::Global(Global::StackLimit)),
            Const(A, limit),
            Store {
                base: B,
                offset: 0,
                from: A,
            },
            StackCheck { bytes, fail },
            Const(A, 0),
            Return,
            Mark(fail),
            Const(A, 1),
            Return,
        ])
    };
    assert_eq!(call(&with_bytes(50), "f", &[]), 0);
    assert_eq!(call(&with_bytes(200), "f", &[]), 1);
}

#[test]
fn run_calls_the_entry_with_no_output() {
    let m = single(vec![Return]);
    let outcome = run(&m, "f", b"");
    assert_eq!(outcome.status, 0);
    assert!(outcome.stdout.is_empty() && outcome.stderr.is_empty());
}
