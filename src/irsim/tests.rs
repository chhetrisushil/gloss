use super::{call, run};
use crate::ir::{
    CWD, Cond, Data, ESCAPES, Function, Global, Inst, IntOp, LINK, Label, Module, NOT_FOUND,
    OpenMode, Prim, Reg, Shift,
};
use crate::number::{NumFail, Number, SCALE};
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
    assert_eq!(
        -drop,
        8 * (1 + 1 + 3),
        "the stack pointer fell by {}",
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

fn dirty_slot_callee() -> Function {
    function(
        "dirty",
        1,
        1,
        vec![LoadSlot(A, 1), LoadArg(B, 1), StoreSlot(1, B), Return],
    )
}

#[test]
fn every_call_gets_fresh_zeroed_slots() {
    let caller = function(
        "caller",
        0,
        1,
        vec![
            Const(A, 9),
            Push(A),
            Call("dirty".to_string()),
            Push(A),
            Const(A, 9),
            Push(A),
            Call("dirty".to_string()),
            Pop(B),
            Int(IntOp::Add),
            Return,
        ],
    );
    let m = two_functions(dirty_slot_callee(), caller);
    assert_eq!(call(&m, "caller", &[]), 0);
}

const ALL_CONDS: [Cond; 6] = [Cond::Eq, Cond::Ne, Cond::Lt, Cond::Le, Cond::Gt, Cond::Ge];

fn expected(cond: Cond, a: i64, b: i64) -> bool {
    match cond {
        Cond::Eq => a == b,
        Cond::Ne => a != b,
        Cond::Lt => a < b,
        Cond::Le => a <= b,
        Cond::Gt => a > b,
        Cond::Ge => a >= b,
    }
}

#[test]
fn every_condition_is_signed_for_set_if_and_jump_if() {
    let taken = Label(0);
    for cond in ALL_CONDS {
        for (a, b) in [(-1, 1), (1, -1), (-2, -2), (-3, -2), (0, 0)] {
            let set = single(vec![Const(A, a), Const(B, b), Compare, SetIf(cond), Return]);
            let want = i64::from(expected(cond, a, b));
            assert_eq!(call(&set, "f", &[]), want, "SetIf {cond:?} {a} {b}");
            let jump = single(vec![
                Const(A, a),
                Const(B, b),
                Compare,
                JumpIf(cond, taken),
                Const(A, 0),
                Return,
                Mark(taken),
                Const(A, 1),
                Return,
            ]);
            assert_eq!(call(&jump, "f", &[]), want, "JumpIf {cond:?} {a} {b}");
        }
    }
}

#[test]
fn a_call_restores_the_stack_pointer_and_leaves_what_was_pushed() {
    let callee = function("callee", 1, 1, vec![Return]);
    let caller = function(
        "caller",
        0,
        2,
        vec![
            Const(A, 77),
            Push(A),
            StackPointer(A),
            StoreSlot(1, A),
            Const(A, 5),
            Push(A),
            Call("callee".to_string()),
            StackPointer(B),
            LoadSlot(A, 1),
            Compare,
            SetIf(Cond::Eq),
            StoreSlot(2, A),
            Pop(A),
            LoadSlot(B, 2),
            Int(IntOp::Mul),
            Return,
        ],
    );
    assert_eq!(call(&two_functions(callee, caller), "caller", &[]), 77);
}

#[test]
fn stack_check_allows_a_frame_that_ends_exactly_at_the_limit() {
    let fail = Label(0);
    // Inside `f` the stack pointer is the top minus its frame's three words.
    let sp = 0x7fff_0000_0000i64 - 24;
    let with_bytes = |bytes| {
        single(vec![
            AddressOf(B, Data::Global(Global::StackLimit)),
            Const(A, sp - 50),
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
    assert_eq!(call(&with_bytes(51), "f", &[]), 1);
}

#[test]
fn and_or_and_stack_memory_work() {
    let and = single(vec![
        Const(A, 0b1100),
        Const(B, 0b1010),
        Int(IntOp::And),
        Return,
    ]);
    assert_eq!(call(&and, "f", &[]), 0b1000);
    let or = single(vec![
        Const(A, 0b1100),
        Const(B, 0b1010),
        Int(IntOp::Or),
        Return,
    ]);
    assert_eq!(call(&or, "f", &[]), 0b1110);
    let stack = single(vec![
        StackPointer(B),
        Const(A, 0x1234),
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
    assert_eq!(call(&stack, "f", &[]), 0x1234);
}

#[test]
#[should_panic(expected = "M2-R7")]
fn dividing_by_zero_panics() {
    call(
        &single(vec![Const(A, 1), Const(B, 0), Int(IntOp::Div), Return]),
        "f",
        &[],
    );
}

#[test]
#[should_panic(expected = "M2-R7")]
fn a_negative_divisor_panics_for_rem() {
    call(
        &single(vec![Const(A, 7), Const(B, -2), Int(IntOp::Rem), Return]),
        "f",
        &[],
    );
}

#[test]
fn a_line_marker_may_come_between_compare_and_its_user() {
    let m = single(vec![
        Const(A, 1),
        Const(B, 2),
        Compare,
        Line(3),
        SetIf(Cond::Lt),
        Return,
    ]);
    assert_eq!(call(&m, "f", &[]), 1);
}

#[test]
#[should_panic(expected = "directly follow Compare")]
fn another_instruction_between_compare_and_set_if_panics() {
    let m = single(vec![
        Const(A, 1),
        Compare,
        Const(B, 2),
        SetIf(Cond::Lt),
        Return,
    ]);
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "directly follow Compare")]
fn one_compare_serves_only_one_instruction() {
    let taken = Label(0);
    let m = single(vec![
        Compare,
        JumpIf(Cond::Ne, taken),
        SetIf(Cond::Eq),
        Mark(taken),
        Return,
    ]);
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "directly follow Compare")]
fn jump_if_without_a_compare_panics() {
    call(
        &single(vec![JumpIf(Cond::Eq, Label(0)), Mark(Label(0)), Return]),
        "f",
        &[],
    );
}

#[test]
#[should_panic(expected = "0x10000000")]
fn writing_a_text_panics() {
    let mut m = single(vec![
        AddressOf(B, Data::Text(0)),
        Store {
            base: B,
            offset: 0,
            from: A,
        },
        Return,
    ]);
    m.text(b"read only");
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "0x0")]
fn reading_address_zero_panics() {
    let m = single(vec![
        Const(B, 0),
        Load {
            to: A,
            base: B,
            offset: 0,
        },
        Return,
    ]);
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "outside mapped memory")]
fn a_word_running_off_the_end_of_a_global_panics() {
    let m = single(vec![
        AddressOf(B, Data::Global(Global::NumBuf)),
        Load {
            to: A,
            base: B,
            offset: 32 - 4,
        },
        Return,
    ]);
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "outside mapped memory")]
fn reading_below_the_stack_pointer_panics() {
    let m = single(vec![
        StackPointer(B),
        Load {
            to: A,
            base: B,
            offset: -8,
        },
        Return,
    ]);
    call(&m, "f", &[]);
}

#[test]
#[should_panic(expected = "outside mapped memory")]
fn reading_above_the_top_of_the_stack_panics() {
    let m = single(vec![
        Const(B, 0x7fff_0000_0000),
        Load {
            to: A,
            base: B,
            offset: 0,
        },
        Return,
    ]);
    call(&m, "f", &[]);
}

/// What a failed `Num*` instruction returns, so a test can tell which label was taken.
const RANGE_TAKEN: i64 = 0x5a5a_0001_0000_0000;
const ZERO_TAKEN: i64 = 0x5a5a_0002_0000_0000;

/// Runs `op` on `a` and `b`, giving the result in `a` or which label was taken.
fn run_num(op: Inst, a: i64, b: i64) -> i64 {
    let m = single(vec![
        Const(A, a),
        Const(B, b),
        op,
        Return,
        Mark(Label(1)),
        Const(A, RANGE_TAKEN),
        Return,
        Mark(Label(2)),
        Const(A, ZERO_TAKEN),
        Return,
    ]);
    call(&m, "f", &[])
}

fn reference(result: Result<Number, NumFail>) -> i64 {
    match result {
        Ok(Number(n)) => n,
        Err(NumFail::OutOfRange) => RANGE_TAKEN,
        Err(NumFail::DivideByZero) => ZERO_TAKEN,
    }
}

const SAMPLES: [i64; 9] = [0, 1, -1, 7, -7, 1_000_000, i64::MAX, i64::MIN, i64::MIN + 1];

#[test]
fn num_instructions_match_the_number_reference() {
    let range = Label(1);
    let zero = Label(2);
    for &x in &SAMPLES {
        let (a, nx) = (x, Number(x));
        assert_eq!(run_num(NumNeg { range }, a, 0), reference(nx.checked_neg()));
        assert_eq!(run_num(NumWhole, a, 0), nx.whole().0);
        for &y in &SAMPLES {
            let ny = Number(y);
            let ctx = format!("{x} and {y}");
            assert_eq!(
                run_num(NumAdd { range }, x, y),
                reference(nx.checked_add(ny)),
                "add {ctx}"
            );
            assert_eq!(
                run_num(NumSub { range }, x, y),
                reference(nx.checked_sub(ny)),
                "sub {ctx}"
            );
            assert_eq!(
                run_num(NumMul { range }, x, y),
                reference(nx.mul_round(ny)),
                "mul {ctx}"
            );
            assert_eq!(
                run_num(NumDiv { zero, range }, x, y),
                reference(nx.div_round(ny)),
                "div {ctx}"
            );
            assert_eq!(
                run_num(NumRem { zero }, x, y),
                reference(nx.rem(ny)),
                "rem {ctx}"
            );
        }
    }
}

#[test]
fn dividing_by_zero_takes_the_zero_label() {
    let div = NumDiv {
        zero: Label(2),
        range: Label(1),
    };
    assert_eq!(run_num(div, 5, 0), ZERO_TAKEN);
}

#[test]
fn multiplying_past_the_range_takes_the_range_label() {
    assert_eq!(
        run_num(NumMul { range: Label(1) }, i64::MAX, 2 * SCALE),
        RANGE_TAKEN
    );
}

fn push_const(n: i64) -> [Inst; 2] {
    [Const(A, n), Push(A)]
}

fn push_global(g: Global) -> [Inst; 2] {
    [AddressOf(A, Data::Global(g)), Push(A)]
}

/// Pushes the address of text 0's bytes, which follow its 8-byte length.
fn push_text_bytes() -> [Inst; 3] {
    [AddressOf(A, Data::Text(0)), AddImm(A, 8), Push(A)]
}

fn with_text(body: Vec<Inst>) -> Module {
    let mut m = single(body);
    m.text(b"abc");
    m
}

fn write_text(fd: i64) -> Vec<Inst> {
    let mut body = Vec::new();
    body.extend(push_const(fd));
    body.extend(push_text_bytes());
    body.extend(push_const(3));
    body.push(System(Prim::Write));
    body
}

#[test]
fn write_to_fd_2_lands_in_stderr_and_returns_the_length() {
    let mut body = write_text(2);
    body.push(Return);
    let m = with_text(body);
    assert_eq!(call(&m, "f", &[]), 3);
    let outcome = run(&m, "f", b"");
    assert_eq!(outcome.stderr, b"abc");
    assert!(outcome.stdout.is_empty());
}

#[test]
fn write_to_fd_1_lands_in_stdout() {
    let mut body = write_text(1);
    body.push(Return);
    let outcome = run(&with_text(body), "f", b"");
    assert_eq!(outcome.stdout, b"abc");
    assert!(outcome.stderr.is_empty());
}

#[test]
fn write_to_another_fd_is_ebadf_and_consumes_its_arguments() {
    let mut body = write_text(5);
    body.extend([StackPointer(B), Move { to: A, from: B }, Return]);
    let m = with_text(body);
    let plain = with_text(vec![StackPointer(A), Return]);
    assert_eq!(call(&m, "f", &[]), call(&plain, "f", &[]));
    let mut failing = write_text(5);
    failing.push(Return);
    assert_eq!(call(&with_text(failing), "f", &[]), -9);
}

fn read_into_in_buf(len: i64) -> Vec<Inst> {
    let mut body = Vec::new();
    body.extend(push_const(0));
    body.extend(push_global(Global::InBuf));
    body.extend(push_const(len));
    body.push(System(Prim::Read));
    body
}

#[test]
fn read_copies_stdin_and_the_bytes_can_be_written_back() {
    let mut body = read_into_in_buf(8);
    body.push(Move { to: B, from: A });
    body.extend(push_const(1));
    body.extend(push_global(Global::InBuf));
    body.extend([Push(B), System(Prim::Write), Return]);
    let outcome = run(&single(body), "f", b"hi");
    assert_eq!(outcome.stdout, b"hi");
}

#[test]
fn read_takes_at_most_len_bytes_then_the_rest_then_nothing() {
    let mut body = read_into_in_buf(3);
    body.extend([Push(A), System(Prim::Exit)]);
    assert_eq!(run(&single(body), "f", b"hello").status, 3);
    let mut twice = read_into_in_buf(3);
    twice.extend(read_into_in_buf(3));
    twice.extend([Push(A), System(Prim::Exit)]);
    assert_eq!(run(&single(twice), "f", b"hello").status, 2);
}

#[test]
fn read_returns_0_at_end_of_input() {
    let mut body = read_into_in_buf(8);
    body.extend([Return]);
    assert_eq!(call(&single(body), "f", &[]), 0);
}

fn map_16() -> Vec<Inst> {
    let mut body = push_const(16).to_vec();
    body.push(System(Prim::Map));
    body
}

#[test]
fn map_starts_at_the_base_and_the_next_block_follows() {
    let mut first = map_16();
    first.push(Return);
    assert_eq!(call(&single(first), "f", &[]), 0x4000_0000_0000);
    let mut second = map_16();
    second.extend(map_16());
    second.push(Return);
    assert_eq!(call(&single(second), "f", &[]), 0x4000_0000_0010);
}

#[test]
fn mapped_memory_is_zeroed_and_writable() {
    let mut zeroed = map_16();
    zeroed.extend([
        Move { to: B, from: A },
        Load {
            to: A,
            base: B,
            offset: 8,
        },
        Return,
    ]);
    assert_eq!(call(&single(zeroed), "f", &[]), 0);
    let mut written = map_16();
    written.extend([
        Move { to: B, from: A },
        Const(A, 7),
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
    assert_eq!(call(&single(written), "f", &[]), 7);
}

#[test]
fn map_over_a_tebibyte_gives_0() {
    let mut body = push_const((1 << 40) + 1).to_vec();
    body.extend([System(Prim::Map), Return]);
    assert_eq!(call(&single(body), "f", &[]), 0);
}

#[test]
#[should_panic(expected = "outside mapped memory")]
fn reading_past_a_mapped_block_panics() {
    let mut body = push_const(16).to_vec();
    body.extend([
        System(Prim::Map),
        Move { to: B, from: A },
        Load {
            to: A,
            base: B,
            offset: 16,
        },
        Return,
    ]);
    call(&single(body), "f", &[]);
}

#[test]
fn exit_stops_at_once_with_its_status_from_any_depth() {
    let mut deep = write_text(1);
    deep.extend(push_const(2));
    deep.extend([System(Prim::Exit), Const(A, 99), Return]);
    let mut m = with_text(vec![Call("deep".to_string()), Const(A, 99), Return]);
    m.functions.push(function("deep", 0, 0, deep));
    let outcome = run(&m, "f", b"");
    assert_eq!(outcome.status, 2);
    assert_eq!(outcome.stdout, b"abc");
}

#[test]
fn a_program_that_returns_has_status_0() {
    let m = single(vec![Const(A, 5), Return]);
    assert_eq!(run(&m, "f", b"").status, 0);
}

#[test]
fn stack_size_is_1_mebibyte() {
    let m = single(vec![System(Prim::StackSize), Return]);
    assert_eq!(call(&m, "f", &[]), 1024 * 1024);
}

const POISON: i64 = 0x5EAD_5EAD_5EAD_5EAD;

/// Runs `op` expecting it to jump; gives `a`, or `b` when `read_b` is set, at the jump.
fn after_failure(op: Inst, a: i64, b: i64, read_b: bool) -> i64 {
    let mut failed = vec![Mark(Label(1))];
    if read_b {
        failed.push(Move { to: A, from: B });
    }
    failed.push(Return);
    let mut body = vec![Const(A, a), Const(B, b), op, Const(A, 0), Return];
    body.extend(failed);
    call(&single(body), "f", &[])
}

#[test]
fn a_failed_number_jump_poisons_a_and_leaves_b() {
    let l = Label(1);
    let ops = [
        (NumAdd { range: l }, i64::MAX, 5),
        (NumSub { range: l }, i64::MIN, 5),
        (NumNeg { range: l }, i64::MIN, 5),
        (NumMul { range: l }, i64::MAX, 2 * SCALE),
        (NumDiv { zero: l, range: l }, 5, 0),
        (NumDiv { zero: l, range: l }, i64::MAX, 1),
        (NumRem { zero: l }, 5, 0),
    ];
    for (op, a, b) in ops {
        assert_eq!(after_failure(op.clone(), a, b, false), POISON, "{op:?}");
        assert_eq!(after_failure(op.clone(), a, b, true), b, "{op:?}");
    }
}

#[test]
fn exit_keeps_the_low_8_bits_of_the_status() {
    for (given, status) in [(256, 0), (-1, 255), (2, 2)] {
        let mut body = push_const(given).to_vec();
        body.push(System(Prim::Exit));
        assert_eq!(run(&single(body), "f", b"").status, status);
    }
}

#[test]
fn map_of_nothing_gives_0() {
    let mut body = push_const(0).to_vec();
    body.extend([System(Prim::Map), Return]);
    assert_eq!(call(&single(body), "f", &[]), 0);
}

#[test]
fn read_checks_only_the_bytes_it_copies() {
    let mut body = read_into_in_buf(8192);
    body.push(Return);
    let outcome = run(&single(body), "f", b"hi");
    assert_eq!(outcome.status, 0);
    let mut count = read_into_in_buf(8192);
    count.extend([Push(A), System(Prim::Exit)]);
    assert_eq!(run(&single(count), "f", b"hi").status, 2);
}

/// A module with a record descriptor, whose last word is the last read-only byte.
fn last_descriptor_byte(offset: i32) -> Module {
    let mut descs = Module::default();
    let n = descs.desc(crate::ir::Desc::Number);
    descs.desc(crate::ir::Desc::Record {
        name: "E".into(),
        fields: vec![("f".into(), n)],
    });
    let mut m = single(vec![
        AddressOf(B, Data::Desc(1)),
        LoadByte {
            to: A,
            base: B,
            offset,
        },
        Return,
    ]);
    m.descs = descs.descs;
    m
}

#[test]
fn the_last_byte_of_the_final_descriptor_can_be_read() {
    call(&last_descriptor_byte(8 * 5 - 1), "f", &[]);
}

#[test]
#[should_panic(expected = "outside mapped memory")]
fn one_byte_past_the_final_descriptor_panics() {
    call(&last_descriptor_byte(8 * 5), "f", &[]);
}

#[test]
fn the_files_go_in_and_come_out_of_a_run() {
    let mut files = super::Files::default();
    files.files.insert("a".to_string(), b"a".to_vec());
    let m = single(vec![Const(A, 0), Return]);
    let o = super::run_with(&m, "f", b"", &[], super::Faults::default(), &mut files);
    assert_eq!(o.files, files);
}

/// Pushes the address of text `n`'s bytes.
fn push_text_n(n: u32) -> [Inst; 3] {
    [AddressOf(A, Data::Text(n)), AddImm(A, 8), Push(A)]
}

/// Runs `body` on `files`, which `Exit`s with `a`.
fn run_out(
    files: &mut super::Files,
    texts: &[&[u8]],
    slots: u32,
    mut body: Vec<Inst>,
) -> super::Outcome {
    body.extend([Push(A), System(Prim::Exit)]);
    let mut m = with_slots(slots, body);
    for text in texts {
        m.text(text);
    }
    super::run_with(&m, "f", b"", &[], super::Faults::default(), files)
}

/// Like `run_out`, giving the exit status as a signed byte. The `texts` are paths, so they
/// end with a NUL.
fn run_on(files: &mut super::Files, texts: &[&[u8]], slots: u32, body: Vec<Inst>) -> i64 {
    run_out(files, texts, slots, body).status as u8 as i8 as i64
}

/// `Open(mode)` of text `n` in the folder `dir` puts in `a`; the result is in `a`.
fn open_in(mode: OpenMode, dir: Inst, n: u32) -> Vec<Inst> {
    let mut body = vec![dir, Push(A)];
    body.extend(push_text_n(n));
    if mode == OpenMode::Create {
        body.extend([Const(A, 0o644), Push(A)]);
    }
    body.push(System(Prim::Open(mode)));
    body
}

/// `Open(mode)` of text `n` in the current folder.
fn open_text(mode: OpenMode, n: u32) -> Vec<Inst> {
    open_in(mode, Const(A, CWD), n)
}

/// `prim(slot)` for a primitive with one argument.
fn on_slot(prim: Prim, slot: u32) -> [Inst; 3] {
    [LoadSlot(A, slot), Push(A), System(prim)]
}

#[test]
fn descriptors_start_at_3_and_the_lowest_free_one_is_reused() {
    let mut files = super::Files::default();
    files.files.insert("a".to_string(), Vec::new());
    let mut body = open_text(OpenMode::Read, 0);
    body.push(StoreSlot(1, A));
    body.extend(open_text(OpenMode::Probe, 0));
    body.push(StoreSlot(2, A));
    body.extend(on_slot(Prim::Close, 1));
    body.extend(open_text(OpenMode::Read, 0));
    body.extend([
        Const(B, 16),
        Int(IntOp::Mul),
        LoadSlot(B, 2),
        Int(IntOp::Add),
    ]);
    assert_eq!(run_on(&mut files, &[b"a\0"], 2, body), 3 * 16 + 4);
    let mut body = vec![Const(A, 9), StoreSlot(1, A)];
    body.extend(on_slot(Prim::Close, 1));
    assert_eq!(run_on(&mut files, &[], 1, body), -9);
}

#[test]
fn open_failures_are_the_ir_errors() {
    let mut files = super::Files::default();
    files.links.insert("l".to_string());
    files.folders.insert("d".to_string());
    files.folders.insert("d/m".to_string());
    files.links.insert("d/m/l".to_string());
    files.files.insert("f".to_string(), Vec::new());
    let cases: [(OpenMode, &[u8], i64); 12] = [
        (OpenMode::Read, b"gone\0", NOT_FOUND),
        (OpenMode::Read, b"l\0", LINK),
        (OpenMode::Read, b"d/m/l/x\0", LINK),
        (OpenMode::Read, b"../x\0", ESCAPES),
        (OpenMode::Create, b"no/x\0", NOT_FOUND),
        (OpenMode::Create, b"f/x\0", -20),
        (OpenMode::Read, b"f/x\0", -20),
        (OpenMode::Folder, b"f\0", -20),
        (OpenMode::Read, b"f/\0", -20),
        (OpenMode::Create, b"d\0", -21),
        (OpenMode::Create, b"new/\0", -21),
        (OpenMode::Folder, b"d/m\0", 3),
    ];
    for (mode, path, want) in cases {
        let got = run_on(&mut files, &[path], 0, open_text(mode, 0));
        assert_eq!(got, want, "{}", String::from_utf8_lossy(path));
    }
    assert_eq!(
        run_on(&mut files, &[b".\0"], 0, open_text(OpenMode::Folder, 0)),
        3
    );
    assert_eq!(
        run_on(&mut files, &[b"\0"], 0, open_text(OpenMode::Folder, 0)),
        NOT_FOUND
    );
}

#[test]
fn sync_records_folders_and_files_and_rename_moves_the_file_under_its_descriptor() {
    let mut files = super::Files::default();
    files.folders.insert("d".to_string());
    let mut body = open_text(OpenMode::Folder, 0);
    body.push(StoreSlot(1, A));
    body.extend(open_in(OpenMode::Create, LoadSlot(A, 1), 1));
    body.push(StoreSlot(2, A));
    body.extend(on_slot(Prim::Sync, 1));
    body.extend([LoadSlot(A, 1), Push(A)]);
    body.extend(push_text_n(1));
    body.extend(push_text_n(2));
    body.extend([System(Prim::Rename), StoreSlot(3, A)]);
    body.extend(on_slot(Prim::Sync, 2));
    body.extend([LoadSlot(A, 3)]);
    assert_eq!(run_on(&mut files, &[b"d\0", b"a\0", b"b\0"], 3, body), 0);
    assert_eq!(files.synced, ["d", "d/b"]);
    assert!(files.files.contains_key("d/b") && !files.files.contains_key("d/a"));
}

#[test]
fn only_open_descriptors_other_than_probes_can_be_synced() {
    let mut files = super::Files::default();
    files.files.insert("a".to_string(), Vec::new());
    let mut body = open_text(OpenMode::Probe, 0);
    body.push(StoreSlot(1, A));
    body.extend(on_slot(Prim::Sync, 1));
    assert_eq!(run_on(&mut files, &[b"a\0"], 1, body), -9);
    let mut body = open_text(OpenMode::Read, 0);
    body.push(StoreSlot(1, A));
    body.extend(on_slot(Prim::Close, 1));
    body.extend(on_slot(Prim::Sync, 1));
    assert_eq!(run_on(&mut files, &[b"a\0"], 1, body), -9);
}

#[test]
fn open_edge_inputs_follow_linux() {
    let mut files = super::Files::default();
    files.files.insert("f".to_string(), Vec::new());
    let mut open = |dir: i64, path: &[u8]| {
        let body = open_in(OpenMode::Read, Const(A, dir), 0);
        run_on(&mut files, &[path], 0, body)
    };
    // The path is judged before the descriptor.
    assert_eq!(open(77, b"/f\0"), ESCAPES);
    assert_eq!(open(77, b"\0"), NOT_FOUND);
    assert_eq!(open(77, b"f\0"), -9);
    // Standard descriptors are not folders.
    for fd in 0..3 {
        assert_eq!(open(fd, b"f\0"), -20, "fd {fd}");
    }
    assert_eq!(open(CWD, b"f/.\0"), -20);
}

#[test]
fn renaming_dot_or_dotdot_is_ebusy_and_standard_fds_are_not_folders() {
    let rename = |dir: i64, first: &[u8], second: &[u8]| {
        let mut body = push_const(dir).to_vec();
        body.extend(push_text_n(0));
        body.extend(push_text_n(1));
        body.push(System(Prim::Rename));
        run_on(&mut super::Files::default(), &[first, second], 0, body)
    };
    assert_eq!(rename(CWD, b".\0", b"x\0"), -16);
    assert_eq!(rename(CWD, b"x\0", b"..\0"), -16);
    assert_eq!(rename(1, b"x\0", b"y\0"), -20);
}

#[test]
fn rename_needs_a_folder_descriptor_and_single_names() {
    let mut files = super::Files::default();
    files.files.insert("a".to_string(), Vec::new());
    let rename = |first: &[u8], second: &[u8]| {
        let mut body = push_const(CWD).to_vec();
        body.extend(push_text_n(0));
        body.extend(push_text_n(1));
        body.push(System(Prim::Rename));
        run_on(&mut super::Files::default(), &[first, second], 0, body)
    };
    assert_eq!(rename(b"a/b\0", b"c\0"), -22);
    let mut body = push_const(7).to_vec();
    body.extend(push_text_n(0));
    body.extend(push_text_n(0));
    body.push(System(Prim::Rename));
    assert_eq!(run_on(&mut files, &[b"a\0"], 0, body.clone()), -9);
    // An empty name is refused before the bad folder descriptor, as on Linux.
    assert_eq!(run_on(&mut files, &[b"\0"], 0, body), -2);
}

/// `prim(fd, buf, len)`, with `fd` put in `a` by `fd`.
fn transfer(prim: Prim, fd: Inst, buf: &[Inst], len: i64) -> Vec<Inst> {
    let mut body = vec![fd, Push(A)];
    body.extend_from_slice(buf);
    body.extend(push_const(len));
    body.push(System(prim));
    body
}

/// Opens text 0 for `mode` into slot `slot`.
fn open_slot(mode: OpenMode, n: u32, slot: u32) -> Vec<Inst> {
    let mut body = open_text(mode, n);
    body.push(StoreSlot(slot, A));
    body
}

#[test]
fn writes_append_and_reads_continue_where_they_stopped() {
    let mut files = super::Files::default();
    let mut body = open_slot(OpenMode::Create, 0, 1);
    for text in [1, 2] {
        body.extend(transfer(Prim::Write, LoadSlot(A, 1), &push_text_n(text), 2));
    }
    run_on(&mut files, &[b"f\0", b"ab", b"cd"], 1, body);
    assert_eq!(files.files["f"], b"abcd".to_vec());
    let mut body = open_slot(OpenMode::Read, 0, 1);
    let in_buf = push_global(Global::InBuf);
    for _ in 0..2 {
        body.extend(transfer(Prim::Read, LoadSlot(A, 1), &in_buf, 2));
        body.extend(transfer(Prim::Write, Const(A, 1), &in_buf, 2));
    }
    body.extend(transfer(Prim::Read, LoadSlot(A, 1), &in_buf, 2));
    let out = run_out(&mut files, &[b"f\0"], 1, body);
    assert_eq!((out.stdout, out.status), (b"abcd".to_vec(), 0));
}

#[test]
fn a_descriptor_follows_its_file_through_a_rename_and_keeps_a_replaced_one() {
    let mut files = super::Files::default();
    files.files.insert("b".to_string(), b"old".to_vec());
    let texts: [&[u8]; 4] = [b"a\0", b"b\0", b"xy", b"z"];
    let mut body = open_slot(OpenMode::Read, 1, 1);
    body.extend(open_slot(OpenMode::Create, 0, 2));
    body.extend(transfer(Prim::Write, LoadSlot(A, 2), &push_text_n(2), 2));
    body.extend(push_const(CWD));
    body.extend(push_text_n(0));
    body.extend(push_text_n(1));
    body.push(System(Prim::Rename));
    body.extend(transfer(Prim::Write, LoadSlot(A, 2), &push_text_n(3), 1));
    let in_buf = push_global(Global::InBuf);
    body.extend(transfer(Prim::Read, LoadSlot(A, 1), &in_buf, 8));
    body.extend(transfer(Prim::Write, Const(A, 1), &in_buf, 3));
    let out = run_out(&mut files, &texts, 2, body);
    assert_eq!(out.stdout, b"old");
    assert_eq!(files.files.keys().collect::<Vec<_>>(), ["b"]);
    assert_eq!(files.files["b"], b"xyz".to_vec());
}

/// Runs `body` with `faults`, which exits with `a` as a signed byte.
fn run_faulty(
    files: &mut super::Files,
    faults: super::Faults,
    texts: &[&[u8]],
    body: Vec<Inst>,
) -> i64 {
    let mut m = with_slots(2, [body, vec![Push(A), System(Prim::Exit)]].concat());
    for text in texts {
        m.text(text);
    }
    super::run_with(&m, "f", b"", &[], faults, files).status as u8 as i8 as i64
}

#[test]
fn a_failing_open_gives_eacces_for_its_path_only() {
    let mut files = super::Files::default();
    files.files.insert("g".to_string(), Vec::new());
    let faults = || super::Faults {
        failing_open: Some("f".to_string()),
        ..super::Faults::default()
    };
    let texts: [&[u8]; 2] = [b"f\0", b"g\0"];
    let denied = open_text(OpenMode::Create, 0);
    assert_eq!(run_faulty(&mut files, faults(), &texts, denied), -13);
    assert!(!files.files.contains_key("f"));
    let fine = open_text(OpenMode::Read, 1);
    assert_eq!(run_faulty(&mut files, faults(), &texts, fine), 3);
}

#[test]
fn a_failing_file_write_fails_valid_descriptors_only() {
    let mut files = super::Files::default();
    let faults = || super::Faults {
        failing_file_write: true,
        ..super::Faults::default()
    };
    let buf = push_global(Global::InBuf);
    let mut valid = open_slot(OpenMode::Create, 0, 1);
    valid.extend(transfer(Prim::Write, LoadSlot(A, 1), &buf, 2));
    assert_eq!(run_faulty(&mut files, faults(), &[b"f\0"], valid), -5);
    let unknown = transfer(Prim::Write, Const(A, 9), &buf, 2);
    assert_eq!(run_faulty(&mut files, faults(), &[], unknown), -9);
    let mut wrong_kind = open_slot(OpenMode::Read, 0, 1);
    wrong_kind.extend(transfer(Prim::Write, LoadSlot(A, 1), &buf, 2));
    assert_eq!(run_faulty(&mut files, faults(), &[b"f\0"], wrong_kind), -9);
}

#[test]
fn a_link_renamed_over_a_file_detaches_its_descriptors_which_share_the_bytes() {
    let mut files = super::Files::default();
    files.files.insert("b".to_string(), b"old".to_vec());
    files.links.insert("l".to_string());
    let in_buf = push_global(Global::InBuf);
    let mut body = open_slot(OpenMode::Read, 0, 1);
    body.extend(open_slot(OpenMode::Create, 0, 2));
    body.extend(transfer(Prim::Write, LoadSlot(A, 2), &push_text_n(2), 2));
    body.extend(push_const(CWD));
    body.extend(push_text_n(1));
    body.extend(push_text_n(0));
    body.push(System(Prim::Rename));
    body.extend(transfer(Prim::Write, LoadSlot(A, 2), &push_text_n(3), 1));
    body.extend(transfer(Prim::Read, LoadSlot(A, 1), &in_buf, 8));
    body.extend(transfer(Prim::Write, Const(A, 1), &in_buf, 3));
    let out = run_out(&mut files, &[b"b\0", b"l\0", b"xy", b"z"], 2, body);
    assert_eq!(out.stdout, b"xyz");
    assert!(files.files.is_empty() && files.links.contains("b"));
}

#[test]
fn reading_a_folder_is_eisdir_and_only_a_folder_or_probe_of_one_is_a_dirfd() {
    let mut files = super::Files::default();
    files.folders.insert("d".to_string());
    files.files.insert("d/x".to_string(), Vec::new());
    files.files.insert("f".to_string(), Vec::new());
    let in_buf = push_global(Global::InBuf);
    let read = |mode, prim, path: &'static [u8]| {
        let mut body = open_slot(mode, 0, 1);
        body.extend(transfer(prim, LoadSlot(A, 1), &in_buf, 1));
        (path, body)
    };
    let cases = [
        read(OpenMode::Read, Prim::Read, b"d\0"),
        read(OpenMode::Folder, Prim::Read, b"d\0"),
        read(OpenMode::Probe, Prim::Read, b"d\0"),
        read(OpenMode::Folder, Prim::Write, b"d\0"),
    ];
    let wants = [-21, -21, -9, -9];
    for ((path, body), want) in cases.into_iter().zip(wants) {
        assert_eq!(run_on(&mut files, &[path], 1, body), want);
    }
    let dirfd = |mode, path: &'static [u8]| {
        let mut body = open_slot(mode, 0, 1);
        body.extend(open_in(OpenMode::Read, LoadSlot(A, 1), 1));
        run_on(&mut files.clone(), &[path, b"x\0"], 1, body)
    };
    assert_eq!(dirfd(OpenMode::Probe, b"d\0"), 4);
    assert_eq!(dirfd(OpenMode::Read, b"d\0"), 4);
    assert_eq!(dirfd(OpenMode::Probe, b"f\0"), -20);
    assert_eq!(dirfd(OpenMode::Read, b"f\0"), -20);
    assert_eq!(dirfd(OpenMode::Create, b"g\0"), -20);
}

#[test]
fn a_probe_of_a_folder_is_a_dirfd_for_rename_and_closing_standard_fds_does_nothing() {
    let mut files = super::Files::default();
    files.folders.insert("d".to_string());
    files.files.insert("d/x".to_string(), Vec::new());
    let mut body = open_slot(OpenMode::Probe, 0, 1);
    body.extend([LoadSlot(A, 1), Push(A)]);
    body.extend(push_text_n(1));
    body.extend(push_text_n(2));
    body.push(System(Prim::Rename));
    assert_eq!(run_on(&mut files, &[b"d\0", b"x\0", b"y\0"], 1, body), 0);
    assert!(files.files.contains_key("d/y"));
    let mut body = push_const(1).to_vec();
    body.push(System(Prim::Close));
    assert_eq!(run_on(&mut files, &[], 0, body), 0);
}

#[test]
fn a_read_only_folder_refuses_create_directly_in_it_only() {
    let mut files = super::Files::default();
    files.folders.insert("d".to_string());
    let faults = || super::Faults {
        read_only_folders: ["d".to_string()].into(),
        ..super::Faults::default()
    };
    let texts: [&[u8]; 2] = [b"d/f\0", b"g\0"];
    let denied = open_text(OpenMode::Create, 0);
    assert_eq!(run_faulty(&mut files, faults(), &texts, denied), -13);
    assert!(!files.files.contains_key("d/f"));
    let fine = open_text(OpenMode::Create, 1);
    assert_eq!(run_faulty(&mut files, faults(), &texts, fine), 3);
    assert!(files.files.contains_key("g"));
}

/// Writes the one byte of text `n` to `fd`; text 0 is "c", 1 is "p", 2 is "e", 3 is "f".
fn say(n: u32, fd: i64) -> Vec<Inst> {
    let mut body = Vec::new();
    body.extend(push_const(fd));
    body.extend(push_text_n(n));
    body.extend(push_const(1));
    body.push(System(Prim::Write));
    body
}

fn exit_with(status: i64) -> Vec<Inst> {
    vec![Const(A, status), Push(A), System(Prim::Exit)]
}

/// `Fork` into slot `slot`; the child, whose result is 0, continues at `child`.
fn fork_into(slot: u32, child: u32) -> Vec<Inst> {
    vec![
        System(Prim::Fork),
        StoreSlot(slot, A),
        Const(B, 0),
        Compare,
        JumpIf(Cond::Eq, Label(child)),
    ]
}

/// `Wait` for the pid in `slot`; the status is in `a`.
fn wait_for(slot: u32) -> Vec<Inst> {
    on_slot(Prim::Wait, slot).to_vec()
}

fn forking(slots: u32, body: Vec<Inst>) -> super::Outcome {
    forking_on(b"", slots, body)
}

fn forking_on(stdin: &[u8], slots: u32, body: Vec<Inst>) -> super::Outcome {
    let mut m = with_slots(slots, body);
    for text in [b"c\0", b"p\0", b"e\0", b"f\0"] {
        m.text(text);
    }
    run(&m, "f", stdin)
}

#[test]
fn a_child_gets_zero_and_the_parent_the_pid() {
    let mut body = fork_into(1, 1);
    body.extend(say(1, 1));
    body.extend(wait_for(1));
    body.extend([Push(A), System(Prim::Exit), Mark(Label(1))]);
    body.extend(say(0, 1));
    body.extend(exit_with(7));
    let outcome = forking(1, body);
    assert_eq!(outcome.stdout, b"cp");
    assert_eq!(outcome.status, 7);
}

#[test]
fn a_child_cannot_change_the_parents_memory() {
    let cell = |from| Store {
        base: B,
        offset: 0,
        from,
    };
    let mut body = vec![
        AddressOf(B, Data::Global(Global::OutBuf)),
        Const(A, 5),
        cell(A),
    ];
    body.extend(fork_into(1, 1));
    body.extend(wait_for(1));
    body.extend([
        AddressOf(B, Data::Global(Global::OutBuf)),
        Load {
            to: A,
            base: B,
            offset: 0,
        },
        Push(A),
        System(Prim::Exit),
        Mark(Label(1)),
        AddressOf(B, Data::Global(Global::OutBuf)),
        Const(A, 99),
        cell(A),
    ]);
    body.extend(exit_with(0));
    assert_eq!(forking(1, body).status, 5);
}

#[test]
fn a_childs_files_and_output_reach_the_parent() {
    let mut body = fork_into(1, 1);
    body.extend(wait_for(1));
    body.extend([Push(A), System(Prim::Exit), Mark(Label(1))]);
    body.extend(open_text(OpenMode::Create, 3));
    body.extend(say(2, 2));
    body.extend(exit_with(0));
    let outcome = forking(1, body);
    assert_eq!(outcome.stderr, b"e");
    assert!(outcome.files.files.contains_key("f"));
}

#[test]
fn waiting_for_an_unknown_child_is_an_error() {
    let mut body = push_const(5).to_vec();
    body.extend([System(Prim::Wait), Push(A), System(Prim::Exit)]);
    assert_eq!(
        forking(0, body).status as u8 as i8 as i64,
        super::system::ECHILD
    );
}

#[test]
#[should_panic(expected = "irsim: wait for a pid of 0 or less is not modelled")]
fn waiting_for_a_pid_of_zero_panics() {
    let mut body = push_const(0).to_vec();
    body.push(System(Prim::Wait));
    forking(0, body);
}

#[test]
fn a_child_has_no_children_of_its_own() {
    // The second child waits for the first, which is its sibling: -10, as on Linux.
    let mut body = fork_into(1, 1);
    body.extend(fork_into(2, 2));
    body.extend(wait_for(2));
    body.extend([Push(A), System(Prim::Exit), Mark(Label(1))]);
    body.extend(exit_with(3));
    body.push(Mark(Label(2)));
    body.extend(wait_for(1));
    body.extend([Push(A), System(Prim::Exit)]);
    assert_eq!(forking(2, body).status, 246);
}

#[test]
fn two_children_wait_in_any_order() {
    let mut body = fork_into(1, 1);
    body.extend(fork_into(2, 2));
    body.extend(wait_for(2));
    body.push(StoreSlot(3, A));
    body.extend(wait_for(1));
    body.extend([
        StoreSlot(4, A),
        LoadSlot(A, 3),
        Shift(crate::ir::Shift::Left, 4),
        LoadSlot(B, 4),
        Int(IntOp::Or),
        Push(A),
        System(Prim::Exit),
        Mark(Label(1)),
    ]);
    body.extend(exit_with(3));
    body.push(Mark(Label(2)));
    body.extend(exit_with(4));
    assert_eq!(forking(4, body).status, 0x43);
}

#[test]
fn a_status_is_forgotten_once_waited_for() {
    let mut body = fork_into(1, 1);
    body.extend(wait_for(1));
    body.extend(wait_for(1));
    body.extend([Push(A), System(Prim::Exit), Mark(Label(1))]);
    body.extend(exit_with(7));
    assert_eq!(
        forking(1, body).status as u8 as i8 as i64,
        super::system::ECHILD
    );
}

#[test]
fn a_childs_reads_move_the_parents_input_position() {
    let mut body = fork_into(1, 1);
    body.extend(read_into_in_buf(3));
    body.extend(push_const(1));
    body.extend(push_global(Global::InBuf));
    body.extend(push_const(2));
    body.extend([System(Prim::Write)]);
    body.extend(exit_with(0));
    body.push(Mark(Label(1)));
    body.extend(read_into_in_buf(3));
    body.extend(exit_with(0));
    assert_eq!(forking_on(b"hello", 1, body).stdout, b"lo");
}

#[test]
#[should_panic(expected = "irsim: a forked child returned")]
fn a_child_returning_from_the_entry_panics() {
    let mut body = fork_into(1, 1);
    body.extend(exit_with(0));
    body.extend([Mark(Label(1)), Return]);
    forking(1, body);
}

#[test]
#[should_panic(expected = "irsim: fork with open files")]
fn forking_with_an_open_descriptor_panics() {
    let mut body = open_text(OpenMode::Create, 3);
    body.push(System(Prim::Fork));
    forking(0, body);
}

#[test]
fn a_grandchild_does_not_reuse_a_pid() {
    let mut body = fork_into(1, 1);
    body.extend(fork_into(2, 2));
    body.extend([
        LoadSlot(A, 2),
        Const(B, 100),
        Int(IntOp::Sub),
        Push(A),
        System(Prim::Exit),
    ]);
    body.push(Mark(Label(1)));
    body.extend(fork_into(3, 3));
    body.extend(exit_with(0));
    body.push(Mark(Label(3)));
    body.extend(exit_with(0));
    body.push(Mark(Label(2)));
    body.extend(exit_with(0));
    assert_eq!(forking(3, body).status, 2);
}
