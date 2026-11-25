//! Tests of the test backend, run on hand-made modules under `irsim`.

use super::tests::{program_for, run};
use super::{Backend, routines_for, test_backend};
use crate::ir::Reg::A;
use crate::ir::{Data, Inst, Inst::*};

/// Pushes the runtime text `i`'s address.
fn text(i: u32) -> Vec<Inst> {
    vec![AddressOf(A, Data::Text(i)), Push(A)]
}

fn number(n: i64) -> Vec<Inst> {
    vec![Const(A, n), Push(A)]
}

/// Calls `routine` with the arguments.
fn call(routine: &str, args: &[Vec<Inst>]) -> Vec<Inst> {
    let mut body = args.concat();
    body.push(Call(routine.into()));
    body
}

/// Runs `body` after `test_begin("t")`, under the Test backend; texts 1 and up are `texts`.
fn run_test(body: &[Vec<Inst>], texts: &[&[u8]]) -> (String, String, i32) {
    let all: Vec<&[u8]> = [&[&b"t"[..]][..], texts].concat();
    let body = [call("test_begin", &[text(0)]), body.concat()].concat();
    let o = run(&program_for(Backend::Test, body, &all));
    let (out, err) = (o.stdout, o.stderr);
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
        o.status,
    )
}

#[test]
fn write_all_sends_descriptor_1_to_shown_and_2_to_the_failure_text() {
    let write = |fd, i| {
        let bytes = [AddressOf(A, Data::Text(i)), AddImm(A, 8), Push(A)];
        call("write_all", &[number(fd), bytes.to_vec(), number(3)])
    };
    let body = [
        write(1, 1),
        write(2, 2),
        write(1, 1),
        call("test_fail", &[]),
    ];
    let (out, err, status) = run_test(&body, &[b"hi\n", b"oops"]);
    let want = "fail  t\n      oop\n      shown:\n        hi\n        hi\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn test_fail_flushes_pending_output_into_shown() {
    let show = call("out_text", &[text(1)]);
    let (out, _, status) = run_test(&[show, call("test_fail", &[])], &[&[b'x'; 300]]);
    assert_eq!(
        (out, status),
        (
            format!("fail  t\n      shown:\n        {}\n", "x".repeat(300)),
            1
        )
    );
}

#[test]
fn no_shown_part_when_nothing_was_shown() {
    let (out, _, status) = run_test(&[call("test_fail", &[])], &[]);
    assert_eq!((out.as_str(), status), ("fail  t\n", 1));
}

#[test]
fn write_all_to_other_descriptors_is_ignored() {
    let at = [AddressOf(A, Data::Text(1)), AddImm(A, 8), Push(A)];
    let mut writes = Vec::new();
    for fd in [0, 3, 9, -1] {
        writes.extend(call("write_all", &[number(fd), at.to_vec(), number(2)]));
    }
    let (out, err, status) = run_test(&[writes, call("test_fail", &[])], &[b"ab"]);
    assert_eq!((out.as_str(), err.as_str(), status), ("fail  t\n", "", 1));
}

#[test]
fn replaced_routines_keep_their_parameters() {
    let real = routines_for(Backend::Real);
    let test = routines_for(Backend::Test);
    for name in test_backend::REPLACED {
        let params =
            |all: &[crate::ir::Function]| all.iter().find(|f| f.name == name).map(|f| f.params);
        assert!(params(&real).is_some(), "{name}");
        assert_eq!(params(&real), params(&test), "{name}");
    }
}
