//! Tests of the test state's routines (`fail_add`, `shown_add`).

use super::test_state::*;
use super::tests::{program, run};
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Global, Inst, Inst::*, IntOp, Prim};

fn state() -> Inst {
    AddressOf(A, Data::Global(Global::TestState))
}

/// Loads word `i` of the test state into `A`.
fn word(i: u32) -> Vec<Inst> {
    let offset = i as i32 * 8;
    vec![
        state(),
        Load {
            to: A,
            base: A,
            offset,
        },
    ]
}

fn set_word(i: u32, n: i64) -> Vec<Inst> {
    let offset = i as i32 * 8;
    let store = Store {
        base: A,
        offset,
        from: B,
    };
    vec![state(), Const(B, n), store]
}

/// Calls `routine(text 0's bytes, len)`.
fn add(routine: &str, len: usize) -> Vec<Inst> {
    let text = [AddressOf(A, Data::Text(0)), AddImm(A, 8), Push(A)];
    let len = [Const(A, len as i64), Push(A), Call(routine.into())];
    [&text[..], &len].concat()
}

/// Writes the `len` bytes at `addr` (each ends in the value in `A`) to standard output.
fn dump(mut addr: Vec<Inst>, mut len: Vec<Inst>) -> Vec<Inst> {
    addr.push(Push(A));
    len.push(Push(A));
    [addr, len, vec![Call("report_bytes".into())]].concat()
}

fn failure_text() -> Vec<Inst> {
    dump(
        vec![state(), AddImm(A, FAIL_BYTES as i32 * 8)],
        word(FAIL_LEN),
    )
}

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

/// The status of `body` followed by an exit with 1 when it has allocated, else 0 (the heap
/// starts empty).
fn allocated(mut body: Vec<Inst>, text: &[u8]) -> i32 {
    let heap = AddressOf(A, Data::Global(Global::HeapNext));
    let load = Load {
        to: A,
        base: A,
        offset: 0,
    };
    body.extend([heap, load, Const(B, 0), Compare, SetIf(Cond::Ne)]);
    body.extend([Push(A), System(Prim::Exit)]);
    run(&program(body, &[text])).status
}

#[test]
fn fail_add_keeps_the_first_1792_bytes() {
    let text = pattern(3000);
    // A full buffer takes nothing more.
    let body = [
        add("fail_add", 2),
        add("fail_add", 3000),
        add("fail_add", 5),
    ];
    let body = [&body.concat()[..], &failure_text()].concat();
    let o = run(&program(body, &[&text]));
    let mut want = text[..2].to_vec();
    want.extend(&text[..1790]);
    assert_eq!((o.status, o.stdout), (0, want));
}

#[test]
fn fail_add_never_allocates() {
    let text = pattern(3000);
    assert_eq!(allocated(add("fail_add", 3000), &text), 0);
    // The check can tell: an allocation moves the heap.
    let alloc = [
        Const(A, 8),
        Push(A),
        Const(A, 0),
        Push(A),
        Call("alloc".into()),
    ];
    assert_eq!(allocated(alloc.to_vec(), &text), 1);
}

/// The shown bytes, written to standard output.
fn shown_bytes() -> Vec<Inst> {
    let at = [word(SHOWN), vec![AddImm(A, 8)]].concat();
    dump(at, word(SHOWN_LEN))
}

#[test]
fn shown_add_collects_and_grows() {
    let text = pattern(9000);
    let body = [add("shown_add", 300), add("shown_add", 5000), shown_bytes()];
    let o = run(&program(body.concat(), &[&text]));
    assert_eq!(
        (o.status, o.stdout),
        (0, [&text[..300], &text[..5000]].concat())
    );
    // A first call grows the buffer past its 4096 bytes of room, so that 4096 stay free.
    let after = |n, size: &[Inst]| {
        let exit = [
            Shift(crate::ir::Shift::Right, 12),
            Push(A),
            System(Prim::Exit),
        ];
        let body = [add("shown_add", n), size.to_vec(), exit.to_vec()];
        run(&program(body.concat(), &[&text])).status
    };
    let free = [
        word(SHOWN_LEN),
        vec![Move { to: B, from: A }],
        word(SHOWN_ROOM),
    ]
    .concat();
    let free = [free, vec![Int(IntOp::Sub)]].concat();
    let rooms = [1, 4100, 9000].map(|n| after(n, &word(SHOWN_ROOM)));
    assert_eq!(
        (rooms, [1, 4100, 9000].map(|n| after(n, &free))),
        ([2, 2, 3], [1; 3])
    );
    // The block's own length word follows.
    let at = Load {
        to: A,
        base: A,
        offset: 0,
    };
    let body = [
        add("shown_add", 7),
        word(SHOWN),
        vec![at, Push(A), System(Prim::Exit)],
    ];
    assert_eq!(run(&program(body.concat(), &[&text])).status, 7);
}

fn set_heap_next_0() -> Vec<Inst> {
    let at = AddressOf(A, Data::Global(Global::HeapNext));
    let zero = Store {
        base: A,
        offset: 0,
        from: B,
    };
    vec![Const(B, 0), at, zero]
}

/// `test_begin` (which allocates the shown buffer), then 250 bytes shown, then `FAILING`
/// with the room cut to 256: a full buffer, which only the guard in `shown_add` can reach.
fn shown_then_failing() -> Vec<Inst> {
    let begin = [
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("test_begin".into()),
    ];
    [
        &begin[..],
        &add("shown_add", 250),
        &set_word(SHOWN_ROOM, 256),
        &set_word(FAILING, 1),
    ]
    .concat()
}

#[test]
fn shown_add_drops_when_failing_and_full() {
    let text = pattern(300);
    let body = [shown_then_failing(), add("shown_add", 300), shown_bytes()].concat();
    let o = run(&program(body, &[&text]));
    assert_eq!((o.status, o.stdout), (0, text[..250].to_vec()));
    // With no buffer at all, nothing is allocated either.
    let body = [set_word(FAILING, 1), add("shown_add", 5)].concat();
    assert_eq!(allocated(body, &text), 0);
    // From a heap that looks empty again, nothing is allocated.
    let empty = set_heap_next_0();
    let body = [shown_then_failing(), empty, add("shown_add", 300)].concat();
    assert_eq!(allocated(body, &text), 0);
    // Not full: the bytes are kept while failing.
    let body = [shown_then_failing(), add("shown_add", 6), shown_bytes()].concat();
    let o = run(&program(body, &[&text]));
    assert_eq!(o.stdout, [&text[..250], &text[..6]].concat());
}

#[test]
fn failing_with_an_allocated_empty_buffer_keeps_bytes() {
    let begin = [
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("test_begin".into()),
    ];
    let body = [
        &begin[..],
        &set_word(FAILING, 1),
        &add("shown_add", 5),
        &shown_bytes(),
    ];
    let o = run(&program(body.concat(), &[b"hello"]));
    assert_eq!(o.stdout, b"hello");
}

#[test]
fn shown_add_of_no_bytes_allocates_nothing() {
    assert_eq!(allocated(add("shown_add", 0), b"x"), 0);
    assert_eq!(allocated(add("shown_add", 1), b"x"), 1);
}
