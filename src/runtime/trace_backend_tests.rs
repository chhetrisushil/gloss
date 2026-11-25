//! Tests of the trace backend's routines: `out_bytes` and its two ways of writing, and the
//! failure event.

use super::Backend;
use super::tests::{program_for, run};
use super::trace_state::{ESCAPING, OPEN};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst, Inst::*, Prim};

/// Sets the word of the trace state at `offset` to `n`.
fn word(offset: i32, n: i64) -> Vec<Inst> {
    vec![
        AddressOf(A, Data::Global(Global::TraceState)),
        Const(B, n),
        Store {
            base: A,
            offset,
            from: B,
        },
    ]
}

fn escaping(n: i64) -> Vec<Inst> {
    word(ESCAPING, n)
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

#[test]
fn the_trace_out_bytes_escapes_with_the_real_json_bytes_when_escaping() {
    let body = [escaping(1), out_then_flush(5)].concat();
    let o = run(&program_for(Backend::Trace, body, &[b"a\"\xC3\xA9\x80"]));
    assert_eq!(
        (o.status, o.stdout),
        (0, "a\\\"é\u{FFFD}".as_bytes().to_vec())
    );
}

/// Pushes the address of text `index` (a text value), as the failure pieces take it.
fn text(index: u32) -> Vec<Inst> {
    vec![AddressOf(A, Data::Text(index)), Push(A)]
}

/// `fail_begin(line, code)`, then `middle`, then `fail_end()`.
fn failing(line: i64, code: i64, middle: Vec<Inst>) -> Vec<Inst> {
    let mut body = vec![
        Const(A, line),
        Push(A),
        Const(A, code),
        Push(A),
        Call("fail_begin".into()),
    ];
    body.extend(middle);
    body.push(Call("fail_end".into()));
    body
}

fn err_text(index: u32) -> Vec<Inst> {
    [text(index), vec![Call("err_text".into())]].concat()
}

/// Runs `body` in a trace build; `(status, stdout, stderr)` as text.
fn outcome(body: Vec<Inst>, texts: &[&[u8]]) -> (i32, String, String) {
    let o = run(&program_for(Backend::Trace, body, texts));
    let text = |b| String::from_utf8(b).unwrap();
    (o.status, text(o.stdout), text(o.stderr))
}

#[test]
fn a_failure_is_one_fail_event() {
    let got = outcome(failing(5, 1, err_text(0)), &[b"dividing by zero (n is 0)"]);
    let line =
        "{\"line\":5,\"event\":\"fail\",\"failure\":1,\"message\":\"dividing by zero (n is 0)\"}\n";
    assert_eq!(got, (2, line.to_string(), String::new()));
}

#[test]
fn err_int_and_err_number_go_into_the_message() {
    let middle = [
        err_text(0),
        vec![Const(A, -12), Push(A), Call("err_int".into())],
        err_text(1),
        vec![Const(A, 1_205_000), Push(A), Call("err_number".into())],
    ]
    .concat();
    let got = outcome(failing(3, 2, middle), &[b"n=", b" x="]);
    let line = "{\"line\":3,\"event\":\"fail\",\"failure\":2,\"message\":\"n=-12 x=120.5\"}\n";
    assert_eq!(got, (2, line.to_string(), String::new()));
}

#[test]
fn a_failure_inside_a_show_closes_it_first() {
    // A hand-made open show: its head, then OPEN and ESCAPING set, `a` written, then the failure.
    let head = [text(1), vec![Call("out_text".into())]].concat();
    let open = [word(OPEN, 1), escaping(1)].concat();
    let a = [text(2), vec![Call("out_text".into())]].concat();
    let body = [head, open, a, failing(4, 1, err_text(0))].concat();
    let head = b"{\"line\":4,\"event\":\"show\",\"text\":\"";
    let (status, out, err) = outcome(body, &[b"x", head, b"a"]);
    let want = "{\"line\":4,\"event\":\"show\",\"text\":\"a\"}\n\
                {\"line\":4,\"event\":\"fail\",\"failure\":1,\"message\":\"x\"}\n";
    assert_eq!((status, out.as_str(), err.as_str()), (2, want, ""));
}

#[test]
fn a_quote_in_the_message_is_escaped() {
    let got = outcome(failing(1, 1, err_text(0)), &[b"say \"hi\"\n"]);
    let line = "{\"line\":1,\"event\":\"fail\",\"failure\":1,\"message\":\"say \\\"hi\\\"\\n\"}\n";
    assert_eq!(got, (2, line.to_string(), String::new()));
}

#[test]
fn a_fail_end_without_a_fail_begin_is_the_real_one() {
    // The internal error of a feature that ends without giving: no event, no `"}`.
    let body = [err_text(0), vec![Call("fail_end".into())]].concat();
    let (status, out, err) = outcome(body, &[b"internal error"]);
    assert_eq!(
        (status, out.as_str(), err.as_str()),
        (2, "", "internal error\n")
    );
}

/// The message a failure of `pieces`, each written by one `err_text`, gives in a trace build.
fn message(pieces: &[&[u8]]) -> String {
    let middle: Vec<Inst> = (0..pieces.len() as u32).flat_map(err_text).collect();
    let (status, out, err) = outcome(failing(1, 5, middle), pieces);
    assert_eq!((status, err.as_str()), (2, ""));
    let head = "{\"line\":1,\"event\":\"fail\",\"failure\":5,\"message\":\"";
    out.strip_prefix(head)
        .and_then(|m| m.strip_suffix("\"}\n"))
        .expect("one fail event")
        .to_string()
}

#[test]
fn a_character_cut_between_two_pieces_arrives_whole() {
    for text in ["aé", "a€b", "a😀b"] {
        let bytes = text.as_bytes();
        for cut in 1..bytes.len() {
            assert_eq!(
                message(&[&bytes[..cut], &bytes[cut..]]),
                text,
                "{text} at {cut}"
            );
        }
        let singles: Vec<&[u8]> = bytes.chunks(1).collect();
        assert_eq!(message(&singles), text, "{text} a byte at a time");
    }
}
