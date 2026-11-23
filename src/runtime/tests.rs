use super::add_reachable;
use crate::ir::{
    Data, Function, Global, Inst,
    Inst::*,
    IntOp, Module, Prim,
    Reg::{A, B},
};
use crate::irsim::{self, Faults, Outcome};

/// A module whose `_start` is `body` followed by `exit 0`, with the runtime it calls.
fn program(body: Vec<Inst>, texts: &[&[u8]]) -> Module {
    let mut m = Module::default();
    for t in texts {
        m.text(t);
    }
    let mut code = vec![Enter(0)];
    code.extend(body);
    code.extend([Const(A, 0), Push(A), System(Prim::Exit)]);
    let called: Vec<String> = code
        .iter()
        .filter_map(|i| match i {
            Call(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    m.functions.push(Function {
        name: "_start".into(),
        params: 0,
        slots: 0,
        body: code,
    });
    add_reachable(&mut m, &called);
    m
}

fn run(m: &Module) -> Outcome {
    irsim::run(m, "_start", b"")
}

/// Calls `write_all(fd, text 0's bytes, len)`.
fn write_text(fd: i64, len: i64) -> Vec<Inst> {
    vec![
        Const(A, fd),
        Push(A),
        AddressOf(A, Data::Text(0)),
        AddImm(A, 8),
        Push(A),
        Const(A, len),
        Push(A),
        Call("write_all".into()),
    ]
}

#[test]
fn write_all_writes_every_byte_to_the_descriptor_given() {
    let m = program(write_text(1, 5000), &[&[b'x'; 5000]]);
    let o = run(&m);
    assert_eq!((o.status, o.stdout.len(), o.stderr.len()), (0, 5000, 0));
    let o = run(&program(write_text(2, 3), &[b"abcdef"]));
    assert_eq!((o.stdout.len(), o.stderr), (0, b"abc".to_vec()));
}

#[test]
fn a_failed_write_ends_the_program_with_status_2() {
    let o = run(&program(write_text(9, 3), &[b"abc"]));
    assert_eq!(o.status, 2);
}

fn out_text(index: u32) -> Vec<crate::ir::Inst> {
    vec![
        AddressOf(A, Data::Text(index)),
        Push(A),
        Call("out_text".into()),
    ]
}

#[test]
fn out_text_then_flush_gives_those_bytes() {
    let text = vec![b'x'; 5000];
    let mut body = out_text(0);
    body.push(Call("flush".into()));
    let o = run(&program(body, &[&text]));
    assert_eq!(o.status, 0);
    assert_eq!(o.stdout, text);
}

#[test]
fn small_writes_fill_and_cross_the_buffer_in_order() {
    let mut want = Vec::new();
    let mut body = Vec::new();
    for _ in 0..300 {
        body.extend(out_text(0));
        body.extend(out_text(1));
        want.extend(b"hello, ");
        want.extend(b"world ");
    }
    body.push(Call("flush".into()));
    let o = run(&program(body, &[b"hello, ", b"world "]));
    assert_eq!(o.stdout, want);
}

#[test]
fn a_write_longer_than_the_buffer_keeps_the_order_of_what_came_before() {
    let long = vec![b'L'; 9000];
    let mut body = out_text(0);
    body.extend(out_text(1));
    body.extend(out_text(0));
    body.push(Call("flush".into()));
    let o = run(&program(body, &[b"ab", &long]));
    let mut want = b"ab".to_vec();
    want.extend(&long);
    want.extend(b"ab");
    assert_eq!(o.stdout, want);
}

#[test]
fn only_the_reachable_routines_are_added_once_each() {
    let mut m = Module::default();
    add_reachable(
        &mut m,
        &["out_text".into(), "out_text".into(), "nothing".into()],
    );
    let mut names: Vec<&str> = m.functions.iter().map(|f| f.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["flush", "out_bytes", "out_text", "write_all"]);
}

#[test]
fn err_text_writes_to_standard_error_unbuffered() {
    let body = vec![
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("err_text".into()),
    ];
    let o = run(&program(body, &[b"oops"]));
    assert_eq!(o.stderr, b"oops");
    assert!(o.stdout.is_empty());
}

#[test]
fn err_int_writes_any_number_in_decimal() {
    let cases = [
        0,
        7,
        10,
        4096,
        100,
        -5,
        -123,
        1_000_000_000_000_000_000,
        i64::MAX,
        i64::MIN,
    ];
    for n in cases {
        let body = vec![Const(A, n), Push(A), Call("err_int".into())];
        assert_eq!(
            run(&program(body, &[])).stderr,
            n.to_string().as_bytes(),
            "{n}"
        );
    }
}

#[test]
fn a_failure_flushes_output_then_writes_its_message_and_exits_2() {
    let mut body = out_text(0);
    body.extend([
        Const(A, 7),
        Push(A),
        Const(A, 3),
        Push(A),
        Call("fail_begin".into()),
        AddressOf(A, Data::Text(1)),
        Push(A),
        Call("err_text".into()),
        Call("fail_end".into()),
    ]);
    let o = run(&program(body, &[b"before", b"boom"]));
    assert_eq!(o.status, 2);
    assert_eq!(o.stdout, b"before");
    assert_eq!(o.stderr, b"line 7: failure 3: boom\n");
}

/// Calls `alloc(bytes, 7)` and stores the result in slot `slot`.
fn alloc(bytes: i64, slot: u32) -> Vec<Inst> {
    vec![
        Const(A, bytes),
        Push(A),
        Const(A, 7),
        Push(A),
        Call("alloc".into()),
        StoreSlot(slot, A),
    ]
}

/// A module whose `main` has `slots` slots, runs `body`, then returns slot 1.
fn returning(slots: u32, body: Vec<Inst>) -> Module {
    let mut m = Module::default();
    let mut code = vec![Enter(slots)];
    code.extend(body);
    code.extend([LoadSlot(A, 1), Return]);
    let called: Vec<String> = code
        .iter()
        .filter_map(|i| match i {
            Call(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    m.functions.push(Function {
        name: "main".into(),
        params: 0,
        slots,
        body: code,
    });
    add_reachable(&mut m, &called);
    m
}

#[test]
fn alloc_gives_aligned_increasing_blocks_across_a_piece_boundary() {
    let mut body = alloc(64 << 20, 1);
    body.extend(alloc(16, 2));
    body.extend(alloc(3, 3));
    body.extend([LoadSlot(A, 2), LoadSlot(B, 1), Int(IntOp::Sub), Return]);
    let m = returning(3, body);
    // The 16 bytes lie in a second piece, after the whole first one.
    assert!(irsim::call(&m, "main", &[]) >= 64 << 20);
    let mut body = alloc(64 << 20, 1);
    body.extend(alloc(16, 2));
    body.extend(alloc(3, 3));
    body.extend([LoadSlot(A, 3), LoadSlot(B, 2), Int(IntOp::Sub), Return]);
    assert_eq!(irsim::call(&returning(3, body), "main", &[]), 16);
    let mut body = alloc(5, 1);
    body.extend(alloc(0, 2));
    body.extend([LoadSlot(A, 2), LoadSlot(B, 1), Int(IntOp::Sub), Return]);
    assert_eq!(irsim::call(&returning(2, body), "main", &[]), 8);
}

#[test]
fn a_block_that_exactly_fills_the_rest_of_a_piece_stays_in_it() {
    // The first piece is 64 MiB; after 8 bytes, 64 MiB - 8 fit exactly, right after them.
    let mut body = alloc(8, 1);
    body.extend(alloc((64 << 20) - 8, 2));
    body.extend([LoadSlot(A, 2), LoadSlot(B, 1), Int(IntOp::Sub), Return]);
    assert_eq!(irsim::call(&returning(2, body), "main", &[]), 8);
}

#[test]
fn blocks_around_the_piece_size_never_overlap_the_next() {
    for bytes in [(64 << 20) - 1, 64 << 20, (64 << 20) + 1] {
        let mut body = alloc(bytes, 1);
        body.extend(alloc(8, 2));
        body.extend([LoadSlot(A, 2), LoadSlot(B, 1), Int(IntOp::Sub), Return]);
        let gap = irsim::call(&returning(2, body), "main", &[]);
        assert!(
            gap >= (bytes + 7) & !7,
            "{bytes}: next block {gap} bytes on"
        );
    }
}

#[test]
fn alloc_hands_out_aligned_nonzero_addresses() {
    let mut body = alloc(1, 1);
    body.extend([LoadSlot(A, 1), Const(B, 7), Int(IntOp::And), Return]);
    assert_eq!(irsim::call(&returning(1, body), "main", &[]), 0);
    let body = alloc(0, 1);
    assert_ne!(irsim::call(&returning(1, body), "main", &[]), 0);
}

#[test]
fn a_failed_map_is_an_out_of_memory_failure() {
    let o = run(&program(alloc(2 << 40, 1), &[]));
    assert_eq!(o.status, 2);
    assert_eq!(o.stderr, b"line 7: failure 10: out of memory\n");
}

#[test]
fn a_size_that_cannot_be_allocated_is_out_of_memory() {
    for bytes in [i64::MAX - 3, i64::MAX - (1 << 40), -16, i64::MAX, i64::MIN] {
        let mut body = alloc(8, 1);
        body.pop(); // `_start` has no slots to store into
        body.extend(alloc(bytes, 2));
        body.extend(out_text(0));
        let o = run(&program(body, &[b"reached"]));
        assert_eq!(o.status, 2, "{bytes}");
        assert!(o.stdout.is_empty(), "{bytes}");
        assert_eq!(o.stderr, b"line 7: failure 10: out of memory\n", "{bytes}");
    }
}

#[test]
fn a_program_that_shows_nothing_exits_0_with_no_output() {
    let mut m = Module::default();
    m.functions.push(Function {
        name: "f_start".into(),
        params: 0,
        slots: 0,
        body: vec![Enter(0), Return],
    });
    add_reachable(&mut m, &["_start".into()]);
    let o = run(&m);
    assert_eq!((o.status, o.stdout.len(), o.stderr.len()), (0, 0, 0));
}

fn limit(size: i64, sp: i64) -> i64 {
    let body = vec![
        Const(A, size),
        Push(A),
        Const(A, sp),
        Push(A),
        Call("stack_limit".into()),
        StoreSlot(1, A),
    ];
    irsim::call(&returning(1, body), "main", &[])
}

#[test]
fn the_stack_limit_is_the_size_below_sp_plus_the_margin() {
    let (sp, kib64) = (0x7fff_0000_0000, 64 << 10);
    assert_eq!(limit(-1, sp), sp - (8 << 20) + kib64);
    assert_eq!(limit(1 << 20, sp), sp - (1 << 20) + kib64);
    assert_eq!(limit(1 << 40, sp), sp - (1 << 30) + kib64);
}

#[test]
fn stack_init_sets_the_limit_below_its_own_stack_pointer() {
    // irsim's StackSize is 8 MiB, so the limit is within a frame of 8 MiB - 64 KiB below.
    let body = vec![
        StackPointer(A),
        StoreSlot(1, A),
        Call("stack_init".into()),
        AddressOf(A, Data::Global(Global::StackLimit)),
        Load {
            to: A,
            base: A,
            offset: 0,
        },
        Move { to: B, from: A },
        LoadSlot(A, 1),
        Int(IntOp::Sub),
        StoreSlot(1, A),
    ];
    let below = irsim::call(&returning(1, body), "main", &[]);
    let want = (8 << 20) - (64 << 10);
    assert!((want..want + 256).contains(&below), "{below}");
}

fn run_with(m: &Module, faults: Faults) -> Outcome {
    irsim::run_with(m, "_start", b"", faults)
}

#[test]
fn write_all_delivers_every_byte_through_short_writes() {
    let faults = Faults {
        short_write: Some(7),
        ..Faults::default()
    };
    let text: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let o = run_with(&program(write_text(1, 5000), &[&text]), faults);
    assert_eq!((o.status, o.stdout), (0, text));
}

#[test]
fn a_failing_standard_output_write_exits_2_with_no_message() {
    let mut body = out_text(0);
    body.push(Call("flush".into()));
    let faults = Faults {
        failing_fds: vec![1],
        ..Faults::default()
    };
    let o = run_with(&program(body, &[b"data"]), faults);
    assert_eq!((o.status, o.stdout.len(), o.stderr.len()), (2, 0, 0));
}

#[test]
fn a_failing_standard_error_write_exits_2() {
    let faults = || Faults {
        failing_fds: vec![2],
        ..Faults::default()
    };
    let say = vec![
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("err_text".into()),
    ];
    assert_eq!(run_with(&program(say, &[b"oops"]), faults()).status, 2);
    let begin = vec![
        Const(A, 1),
        Push(A),
        Const(A, 2),
        Push(A),
        Call("fail_begin".into()),
    ];
    let o = run_with(&program(begin, &[]), faults());
    assert_eq!((o.status, o.stderr.len()), (2, 0));
}

/// A small fixed-seed generator (xorshift64*), so the reference test needs no crate.
fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

#[test]
fn number_text_equals_number_show_for_5000_values() {
    let mut values = vec![
        i64::MIN,
        i64::MAX,
        0,
        1,
        -1,
        9999,
        -9999,
        10000,
        -10000,
        10001,
        -10001,
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15;
    while values.len() < 5000 {
        let r = xorshift(&mut state);
        // Vary the magnitude so short numbers and zero fractions are covered as well.
        values.push((r as i64) >> (xorshift(&mut state) % 64));
    }
    let mut body = Vec::new();
    for v in &values {
        body.extend([Const(A, *v), Push(A), Const(A, 7), Push(A)]);
        body.extend([Call("number_text".into()), Push(A), Call("out_text".into())]);
        body.extend(out_text(0));
    }
    body.push(Call("flush".into()));
    let o = run(&program(body, &[b"\n"]));
    let want: String = values
        .iter()
        .map(|v| format!("{}\n", crate::number::Number(*v).show()))
        .collect();
    assert_eq!(o.status, 0);
    assert_eq!(String::from_utf8(o.stdout).unwrap(), want);
}
