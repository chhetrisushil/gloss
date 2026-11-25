//! Tests of the trace backend's routines: `out_bytes` and its two ways of writing.

use super::Backend;
use super::tests::{program_for, run};
use super::trace_state::ESCAPING;
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst, Inst::*, Prim};

/// Sets the escaping word of the trace state to `n`.
fn escaping(n: i64) -> Vec<Inst> {
    vec![
        AddressOf(A, Data::Global(Global::TraceState)),
        Const(B, n),
        Store {
            base: A,
            offset: ESCAPING,
            from: B,
        },
    ]
}

/// Calls `out_bytes(text 0's bytes, len)`, then `flush`.
fn out_then_flush(len: i64) -> Vec<Inst> {
    vec![
        AddressOf(A, Data::Text(0)),
        AddImm(A, 8),
        Push(A),
        Const(A, len),
        Push(A),
        Call("out_bytes".into()),
        Call("flush".into()),
    ]
}

/// A `json_bytes` that writes the byte `J` to standard output and nothing else.
fn counting_json_bytes() -> Function {
    let body = vec![
        Enter(0),
        Const(A, i64::from(b'J')),
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
        Const(A, 0),
        Return,
    ];
    Function {
        name: "json_bytes".into(),
        params: 2,
        slots: 0,
        body,
    }
}

#[test]
fn the_trace_out_bytes_passes_through_when_not_escaping() {
    let body = [escaping(0), out_then_flush(3)].concat();
    let o = run(&program_for(Backend::Trace, body, &[b"a\"b"]));
    assert_eq!((o.status, o.stdout), (0, b"a\"b".to_vec()));
}

#[test]
fn the_trace_out_bytes_routes_to_json_bytes_when_escaping() {
    let body = [escaping(1), out_then_flush(3)].concat();
    let mut m = program_for(Backend::Trace, body, &[b"a\"b"]);
    let at = m
        .functions
        .iter()
        .position(|f| f.name == "json_bytes")
        .expect("an escaping out_bytes must call json_bytes");
    m.functions[at] = counting_json_bytes();
    let o = run(&m);
    assert_eq!((o.status, o.stdout), (0, b"J".to_vec()));
}
