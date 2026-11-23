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
