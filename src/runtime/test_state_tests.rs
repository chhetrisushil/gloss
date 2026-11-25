//! Tests of the test state's routines (`fail_add`).

use super::test_state::*;
use super::tests::{program, run};
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Global, Inst, Inst::*, Prim};

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
