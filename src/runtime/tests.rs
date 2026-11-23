use super::add_reachable;
use crate::ir::{Data, Function, Inst, Inst::*, Module, Prim, Reg::A};
use crate::irsim::{self, Outcome};

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
