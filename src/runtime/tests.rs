use super::add_reachable;
use crate::ir::{
    Data, Function, Global, Inst,
    Inst::*,
    IntOp, Label, Module, Prim,
    Reg::{A, B},
};
use crate::irsim::{self, Faults, Files, Outcome};

/// A module whose `_start` is `body` followed by `exit 0`, with the runtime it calls.
pub(super) fn program(body: Vec<Inst>, texts: &[&[u8]]) -> Module {
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

pub(super) fn run(m: &Module) -> Outcome {
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

fn run_with(m: &Module, faults: Faults) -> Outcome {
    irsim::run_with(m, "_start", b"", &[], faults, &mut Files::default())
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
pub(super) fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

#[test]
fn number_format_equals_number_show_for_5000_values() {
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
        -5000,
        10,
        50500,
        10001,
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15;
    while values.len() < 5000 {
        let r = xorshift(&mut state);
        // Vary the magnitude so short numbers and zero fractions are covered as well.
        values.push((r as i64) >> (xorshift(&mut state) % 64));
    }
    let mut body = Vec::new();
    for v in &values {
        body.extend([Const(A, *v), Push(A)]);
        body.extend([
            Call("number_format".into()),
            Push(A),
            Call("out_text".into()),
        ]);
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

#[test]
fn err_number_writes_to_standard_error() {
    let body = vec![Const(A, -75_000), Push(A), Call("err_number".into())];
    let o = run(&program(body, &[]));
    assert_eq!((o.stdout.len(), o.stderr), (0, b"-7.5".to_vec()));
}

#[test]
fn err_number_in_a_failure_message_needs_no_memory() {
    // With an empty heap any allocation would map a piece and move heap_next, and a failed
    // map would add a second failure message. Neither happens.
    let mut body = vec![Const(A, 3), Push(A), Const(A, 7), Push(A)];
    body.extend([Call("fail_begin".into()), Const(A, 12_345), Push(A)]);
    body.extend([Call("err_number".into()), Call("fail_end".into())]);
    let o = run(&program(body, &[]));
    assert_eq!(o.status, 2);
    assert_eq!(o.stderr, b"line 3: failure 7: 1.2345\n");
}

#[test]
fn err_number_leaves_the_heap_untouched() {
    // Any allocation, however small, maps a piece and sets heap_end.
    let body = vec![
        Const(A, 12_345),
        Push(A),
        Call("err_number".into()),
        AddressOf(A, Data::Global(Global::HeapNext)),
        Load {
            to: A,
            base: A,
            offset: 0,
        },
        StoreSlot(1, A),
        AddressOf(A, Data::Global(Global::HeapEnd)),
        Load {
            to: A,
            base: A,
            offset: 0,
        },
        StoreSlot(2, A),
        LoadSlot(B, 1),
        Int(IntOp::Add),
        Return,
    ];
    assert_eq!(irsim::call(&returning(2, body), "main", &[]), 0);
}

#[test]
fn out_number_never_moves_the_heap() {
    // Slot 1 counts down from 100000; slot 2 keeps the first heap_next.
    let mut body = vec![Const(A, 100_000), StoreSlot(1, A)];
    body.extend([
        Mark(Label(1)),
        LoadSlot(A, 1),
        Push(A),
        Call("out_number".into()),
    ]);
    body.extend([LoadSlot(A, 1), AddImm(A, -1), StoreSlot(1, A), Const(B, 0)]);
    body.extend([Compare, JumpIf(crate::ir::Cond::Gt, Label(1))]);
    body.extend([
        AddressOf(A, Data::Global(Global::HeapNext)),
        Load {
            to: A,
            base: A,
            offset: 0,
        },
    ]);
    body.extend([StoreSlot(2, A), Return]);
    let m = returning(2, body);
    assert_eq!(irsim::call(&m, "main", &[]), 0);
}

#[test]
fn the_stack_limit_is_measured_from_the_end_of_the_executable_name() {
    // irsim's initial stack puts argc and the name below its top, at an odd offset: the limit
    // is the name's end rounded up to a page, minus 1 MiB, plus 64 KiB.
    let want = irsim::INITIAL_STACK_TOP as i64 - (1 << 20) + (64 << 10);
    let m = program(
        vec![
            Call("stack_init".into()),
            AddressOf(A, Data::Global(Global::StackLimit)),
            Load {
                to: A,
                base: A,
                offset: 0,
            },
            Const(B, want),
            Compare,
            SetIf(crate::ir::Cond::Eq),
            // Exit 7 when it is, else 1.
            Const(B, 6),
            Int(IntOp::Mul),
            AddImm(A, 1),
            Push(A),
            System(Prim::Exit),
        ],
        &[],
    );
    assert_eq!(run(&m).status, 7);
}

/// Runs `stack_init` on an initial stack of `words` words with `faults`; true when the limit
/// is the fallback's: the stack pointer inside `stack_init` (`argc` is `words` words below the
/// page under the stack top, `_start` takes a 3-word frame, `stack_init` 3 + 3 words), minus
/// 1 MiB, plus 64 KiB.
fn falls_back(faults: Faults, words: i64) -> bool {
    let sp = irsim::INITIAL_STACK_TOP as i64 - 4096 - 8 * words - 24 - 48;
    let want = sp - (1 << 20) + (64 << 10);
    let m = program(
        vec![
            Call("stack_init".into()),
            AddressOf(A, Data::Global(Global::StackLimit)),
            Load {
                to: A,
                base: A,
                offset: 0,
            },
            Const(B, want),
            Compare,
            SetIf(crate::ir::Cond::Eq),
            Const(B, 6),
            Int(IntOp::Mul),
            AddImm(A, 1),
            Push(A),
            System(Prim::Exit),
        ],
        &[],
    );
    run_with(&m, faults).status == 7
}

#[test]
fn without_an_executable_name_the_limit_falls_back_to_the_stack_pointer() {
    let faults = Faults {
        no_exec_name: true,
        ..Faults::default()
    };
    assert!(falls_back(faults, 11));
}

#[test]
fn an_auxv_that_runs_past_64_pairs_falls_back() {
    let faults = Faults {
        far_exec_name: true,
        ..Faults::default()
    };
    assert!(falls_back(faults, 11 + 128));
}

#[test]
fn a_name_of_more_than_4096_bytes_falls_back() {
    let faults = Faults {
        long_exec_name: true,
        ..Faults::default()
    };
    assert!(falls_back(faults, 11));
}

fn equal_of(x: u32, y: u32) -> Vec<Inst> {
    vec![
        AddressOf(A, Data::Text(x)),
        Push(A),
        AddressOf(A, Data::Text(y)),
        Push(A),
        Call("text_equal".into()),
        // The answer is 0 or 1; show it as a digit through `out_number`'s fixed point form.
        Const(B, 10_000),
        Int(IntOp::Mul),
        Push(A),
        Call("out_number".into()),
    ]
}

#[test]
fn text_equal_compares_lengths_then_bytes() {
    // The module interns equal texts, so these five are distinct.
    let texts: [&[u8]; 7] = [
        b"",
        b"abc",
        b"abd",
        b"abcd",
        b"xbc",
        b"twenty bytes of text",
        b"twenty bytes of texT",
    ];
    let mut body = Vec::new();
    let pairs = [
        (0, 1),
        (1, 2),
        (1, 3),
        (3, 1),
        (1, 4),
        (1, 1),
        (0, 0),
        (5, 6),
        (5, 5),
    ];
    for (x, y) in pairs {
        body.extend(equal_of(x, y));
    }
    body.push(Call("flush".into()));
    let o = run(&program(body, &texts));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), "000001101");
}

#[test]
fn a_text_built_on_the_heap_equals_the_same_bytes_in_the_program() {
    let body = vec![
        Const(A, 16),
        Push(A),
        Const(A, 0),
        Push(A),
        Call("alloc".into()),
        Const(B, 3),
        Store {
            base: A,
            offset: 0,
            from: B,
        },
        Push(A),
        AddressOf(B, Data::Text(0)),
        Load {
            to: B,
            base: B,
            offset: 8,
        },
        Store {
            base: A,
            offset: 8,
            from: B,
        },
        AddressOf(B, Data::Text(0)),
        Push(B),
        Call("text_equal".into()),
        Const(B, 10_000),
        Int(IntOp::Mul),
        Push(A),
        Call("out_number".into()),
        Call("flush".into()),
    ];
    let o = run(&program(body, &[b"abc"]));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), "1");
}

#[test]
fn number_text_gives_a_fresh_text_that_outlives_the_next_format() {
    // 7 as text, then 99 as text, then show the first: the scratch has been rewritten.
    let mut body = Vec::new();
    for n in [70_000, 990_000] {
        body.extend([
            Const(A, n),
            Push(A),
            Const(A, 1),
            Push(A),
            Call("number_text".into()),
        ]);
        body.push(Push(A));
    }
    body.extend([
        Const(A, 5),
        Push(A),
        Call("number_format".into()),
        Pop(B),
        Pop(A),
        Push(A),
        Push(B),
        Call("out_text".into()),
        Call("out_text".into()),
        Call("flush".into()),
    ]);
    let o = run(&program(body, &[]));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), "997");
}

#[test]
fn put_decimal_appends_zero_padded_digits_and_returns_the_new_length() {
    let cases: [(i64, i64); 9] = [
        (0, 1),
        (7, 1),
        (10, 1),
        (7, 4),
        (123, 2),
        (9999, 4),
        (42, 0),
        (922_337_203_685_477, 1),
        (i64::MAX, 19),
    ];
    let mut body = Vec::new();
    for (value, width) in cases {
        // A buffer holding "x", kept on the stack while `put_decimal` appends after it.
        body.extend([
            Const(A, 64),
            Push(A),
            Const(A, 1),
            Push(A),
            Call("alloc".into()),
        ]);
        body.extend([
            Const(B, i64::from(b'x')),
            StoreByte {
                base: A,
                offset: 8,
                from: B,
            },
            Push(A),
            Push(A),
        ]);
        for n in [1, value, width] {
            body.extend([Const(A, n), Push(A)]);
        }
        body.extend([Call("put_decimal".into()), Pop(B)]);
        body.extend([
            Store {
                base: B,
                offset: 0,
                from: A,
            },
            Push(B),
            Call("out_text".into()),
        ]);
        body.extend(out_text(0));
    }
    body.push(Call("flush".into()));
    let o = run(&program(body, &[b"\n"]));
    let want: String = cases
        .iter()
        .map(|(v, w)| format!("x{v:0w$}\n", w = *w as usize))
        .collect();
    assert_eq!(String::from_utf8(o.stdout).unwrap(), want);
}

#[test]
fn number_places_equals_with_places_for_3000_values_at_every_place_count() {
    use crate::number::{Number, Places};
    let mut values = vec![
        i64::MIN,
        i64::MAX,
        0,
        1,
        -1,
        5,
        -5,
        49,
        -49,
        50,
        -50,
        499,
        -500,
        -9,
    ];
    let mut state = 0xD1B5_4A32_D192_ED03;
    while values.len() < 3000 {
        let r = xorshift(&mut state);
        values.push((r as i64) >> (xorshift(&mut state) % 64));
    }
    let mut body = Vec::new();
    for v in &values {
        for places in 0..=4 {
            body.extend([Const(A, *v), Push(A), Const(A, places), Push(A)]);
            body.extend([Const(A, 1), Push(A), Call("number_places".into())]);
            body.extend([Push(A), Call("out_text".into())]);
            body.extend(out_text(0));
        }
    }
    body.push(Call("flush".into()));
    let o = run(&program(body, &[b"\n"]));
    let mut want = String::new();
    for v in &values {
        for places in 0..=4 {
            let p = Places::new(places).unwrap();
            want.push_str(&format!("{}\n", Number(*v).with_places(p)));
        }
    }
    assert_eq!(o.status, 0);
    assert_eq!(String::from_utf8(o.stdout).unwrap(), want);
}

/// Calls `file_write(fd, text 0's bytes, len)` and exits with its result.
fn file_write_exiting(fd: i64, len: i64) -> Vec<Inst> {
    let mut body = write_text(fd, len);
    *body.last_mut().unwrap() = Call("file_write".into());
    body.extend([Push(A), System(Prim::Exit)]);
    body
}

#[test]
fn file_write_delivers_every_byte_through_short_writes_and_gives_0() {
    let faults = Faults {
        short_write: Some(7),
        ..Faults::default()
    };
    let text: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let o = run_with(&program(file_write_exiting(1, 5000), &[&text]), faults);
    assert_eq!((o.status, o.stdout), (0, text));
}

#[test]
fn file_write_gives_the_error_instead_of_ending_the_program() {
    let m = program(file_write_exiting(9, 3), &[b"abc"]);
    // The exit status keeps the low 8 bits of the result.
    assert_eq!(run(&m).status, i32::from(-9i8 as u8));
    let faults = Faults {
        failing_fds: vec![2],
        ..Faults::default()
    };
    let o = run_with(&program(file_write_exiting(2, 3), &[b"abc"]), faults);
    assert_eq!((o.status, o.stderr.len()), (i32::from(-5i8 as u8), 0));
}

#[test]
fn grow_text_copies_len_bytes_for_len_up_to_twice_the_room() {
    // len 0, len 1, len = room and len = 2 * room (the bound the exact-size cell copy uses).
    for (len, room) in [(0, 4), (1, 4), (4, 4), (8, 4), (7, 4), (1, 1), (2, 1)] {
        let body = vec![
            AddressOf(A, Data::Text(0)),
            Push(A),
            Const(A, len),
            Push(A),
            Const(A, room),
            Push(A),
            Const(A, 7),
            Push(A),
            Call("grow_text".into()),
            AddImm(A, 8),
            Push(A),
            Const(A, len),
            Push(A),
            Call("out_bytes".into()),
            Call("flush".into()),
        ];
        let o = run(&program(body, &[b"abcdefgh"]));
        assert_eq!(o.status, 0);
        assert_eq!(
            o.stdout,
            &b"abcdefgh"[..len as usize],
            "len {len} room {room}"
        );
    }
}

#[test]
fn the_real_backend_is_unchanged() {
    use super::{Backend, routines, routines_for, test_backend};
    let real = routines_for(Backend::Real);
    assert_eq!(real, routines());
    let mut expected: Vec<Function> = real
        .iter()
        .filter(|f| !test_backend::REPLACED.contains(&f.name.as_str()))
        .cloned()
        .collect();
    expected.extend(test_backend::routines());
    assert_eq!(routines_for(Backend::Test), expected);
    let original = super::output::routines()
        .into_iter()
        .find(|f| f.name == "write_all")
        .unwrap();
    let kept = real.iter().find(|f| f.name == "write_all").unwrap();
    assert_eq!(kept, &original);
}

#[test]
fn a_replaced_routine_is_swapped_by_name() {
    use super::{Backend, routines, routines_for, routines_swapping};
    let stub = Function {
        name: "flush".into(),
        params: 0,
        slots: 0,
        body: vec![Return],
    };
    let swapped = routines_swapping(Backend::Test, &["flush"], vec![stub.clone()]);
    let flushes: Vec<&Function> = swapped.iter().filter(|f| f.name == "flush").collect();
    assert_eq!(flushes, vec![&stub]);
    let real = routines_for(Backend::Real);
    assert_eq!(
        routines_swapping(Backend::Real, &["flush"], vec![stub]),
        routines()
    );
    assert_ne!(
        real.iter().find(|f| f.name == "flush").unwrap().body,
        vec![Return]
    );
}

fn stub_named(name: &str) -> Function {
    Function {
        name: name.into(),
        params: 0,
        slots: 0,
        body: vec![Return],
    }
}

#[test]
#[should_panic(expected = "does not replace it")]
fn an_extra_that_collides_with_a_real_routine_it_does_not_replace_is_refused() {
    super::routines_swapping(super::Backend::Test, &[], vec![stub_named("flush")]);
}

#[test]
#[should_panic(expected = "not a real routine")]
fn a_replaced_name_that_is_no_real_routine_is_refused() {
    super::routines_swapping(super::Backend::Test, &["flsuh"], vec![]);
}

/// A `write_all` that writes nothing itself: per call it puts `fd` and `len` (as digits) on
/// standard output, so the output has two bytes per call.
fn counting_write_all() -> Function {
    let mut body = vec![Enter(0)];
    for arg in [1, 3] {
        body.extend([
            LoadArg(A, arg),
            AddImm(A, i32::from(b'0')),
            Push(A),
            Const(A, 1),
            Push(A),
            StackPointer(A),
            AddImm(A, 8),
            Push(A),
            Const(A, 1),
            Push(A),
            System(Prim::Write),
            Pop(B),
        ]);
    }
    body.extend([Const(A, 0), Return]);
    Function {
        name: "write_all".into(),
        params: 3,
        slots: 0,
        body,
    }
}

#[test]
fn err_int_writes_through_write_all() {
    let body = vec![Const(A, -12), Push(A), Call("err_int".into())];
    let mut m = program(body, &[]);
    let at = m
        .functions
        .iter()
        .position(|f| f.name == "write_all")
        .expect("write_all must be in the module: err_int writes through it");
    m.functions[at] = counting_write_all();
    let o = run(&m);
    assert_eq!(o.status, 0);
    assert_eq!(o.stdout, b"212121", "3 calls, fd 2 and length 1 each");
    assert!(o.stderr.is_empty());
}

/// Calls `routine(text 0's bytes, len[, indent])`.
fn report_call(routine: &str, text: &[u8], indent: Option<i64>, faults: Faults) -> Outcome {
    let mut body = write_text(1, text.len() as i64);
    body.pop();
    body.remove(0);
    body.remove(0);
    if let Some(n) = indent {
        body.extend([Const(A, n), Push(A)]);
    }
    body.push(Call(routine.into()));
    run_with(&program(body, &[text]), faults)
}

#[test]
fn report_lines_indents_every_line() {
    for (text, want) in [
        (&b"a\nb\n"[..], "      a\n      b\n"),
        (b"a\nb", "      a\n      b\n"),
        (b"", ""),
        (b"\n", "      \n"),
    ] {
        let o = report_call("report_lines", text, Some(6), Faults::default());
        assert_eq!(
            (o.status, String::from_utf8(o.stdout).unwrap()),
            (0, want.into())
        );
    }
    let o = report_call("report_lines", b"x", Some(8), Faults::default());
    assert_eq!(o.stdout, b"        x\n");
}

#[test]
fn report_bytes_delivers_every_byte_through_short_writes() {
    let faults = Faults {
        short_write: Some(7),
        ..Faults::default()
    };
    let text: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let o = report_call("report_bytes", &text, None, faults);
    assert_eq!((o.status, o.stdout), (0, text));
}

#[test]
fn report_text_bypasses_the_output_buffer() {
    // The buffered text is never flushed here, so only the report reaches descriptor 1.
    let mut body = out_text(1);
    body.extend([
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("report_text".into()),
    ]);
    assert_eq!(run(&program(body, &[b"abc", b"buffered"])).stdout, b"abc");
}

#[test]
fn a_failed_report_write_ends_the_process_with_status_2() {
    let faults = Faults {
        failing_fds: vec![1],
        ..Faults::default()
    };
    assert_eq!(report_call("report_bytes", b"abc", None, faults).status, 2);
}

#[test]
fn report_lines_keeps_blank_lines_and_takes_indent_0_and_long_input() {
    let o = report_call("report_lines", b"a\n\nb", Some(6), Faults::default());
    assert_eq!(o.stdout, b"      a\n      \n      b\n");
    let o = report_call("report_lines", b"x\ny", Some(0), Faults::default());
    assert_eq!(o.stdout, b"x\ny\n");
    let lines: Vec<String> = (0..300).map(|i| format!("line {i}")).collect();
    let text = lines.join("\n");
    let want: String = lines.iter().map(|l| format!("    {l}\n")).collect();
    let o = report_call("report_lines", text.as_bytes(), Some(4), Faults::default());
    assert_eq!((o.status, String::from_utf8(o.stdout).unwrap()), (0, want));
}
