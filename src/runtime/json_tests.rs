//! Tests of the JSON writers: the exact bytes `json_bytes`, `json_int`, `json_text` and
//! `json_value` write for each kind of input.

use super::tests::{program, run};
use crate::ir::Reg::A;
use crate::ir::{Data, Desc, Inst, Inst::*, Module};

/// What `json_bytes` writes for `bytes`, flushed to standard output.
fn escaped(bytes: &[u8]) -> Vec<u8> {
    let body = vec![
        AddressOf(A, Data::Text(0)),
        AddImm(A, 8),
        Push(A),
        Const(A, bytes.len() as i64),
        Push(A),
        Call("json_bytes".into()),
        Call("flush".into()),
    ];
    let o = run(&program(body, &[bytes]));
    assert_eq!(o.status, 0);
    o.stdout
}

#[test]
fn quote_and_backslash_get_a_backslash() {
    assert_eq!(escaped(b"a\"b\\c"), b"a\\\"b\\\\c");
}

#[test]
fn newline_return_and_tab_have_short_forms() {
    assert_eq!(escaped(b"\n\r\t"), b"\\n\\r\\t");
}

#[test]
fn other_control_bytes_and_delete_are_lower_case_u_escapes() {
    assert_eq!(escaped(&[0x01]), b"\\u0001");
    assert_eq!(escaped(&[0x1f]), b"\\u001f");
    assert_eq!(escaped(&[0x7f]), b"\\u007f");
    assert_eq!(escaped(&[0x0b, 0x0a]), b"\\u000b\\n");
}

#[test]
fn valid_multi_byte_sequences_pass_unchanged() {
    assert_eq!(escaped("é".as_bytes()), "é".as_bytes());
    assert_eq!(escaped("€".as_bytes()), "€".as_bytes());
    assert_eq!(escaped("a😀b".as_bytes()), "a😀b".as_bytes());
}

#[test]
fn each_byte_that_starts_no_valid_sequence_becomes_the_replacement_character() {
    assert_eq!(escaped(&[0x80]), "\u{FFFD}".as_bytes());
    assert_eq!(escaped(b"\xC3A"), "\u{FFFD}A".as_bytes());
    assert_eq!(escaped(&[0xC0, 0x80]), "\u{FFFD}\u{FFFD}".as_bytes());
    assert_eq!(escaped(&[0xE2, 0x82]), "\u{FFFD}\u{FFFD}".as_bytes());
}

#[test]
fn nothing_in_gives_nothing_out() {
    assert_eq!(escaped(b""), b"");
}

#[test]
fn five_thousand_escaped_quotes_arrive_whole() {
    assert_eq!(escaped(&[b'"'; 5000]), b"\\\"".repeat(5000));
}

/// What the runtime routine `name` writes when called with `args` pushed first to last, with
/// `texts` as the module's texts and `d` holding the descriptors; flushed to standard output.
fn written(name: &str, args: Vec<Inst>, texts: &[&[u8]], d: Module) -> String {
    let mut body = Vec::new();
    for a in args {
        body.extend([a, Push(A)]);
    }
    body.extend([Call(name.into()), Call("flush".into())]);
    let mut m = program(body, texts);
    m.descs = d.descs;
    let o = run(&m);
    assert_eq!(o.status, 0);
    String::from_utf8(o.stdout).unwrap()
}

/// What `json_value` writes for the scalar `value` of the type `kind` (descriptor 0).
fn scalar(value: Inst, kind: Desc, texts: &[&[u8]]) -> String {
    let mut d = Module::default();
    d.desc(kind);
    written(
        "json_value",
        vec![value, AddressOf(A, Data::Desc(0))],
        texts,
        d,
    )
}

#[test]
fn a_number_is_its_show_form_in_a_string() {
    let number = |n| scalar(Const(A, n), Desc::Number, &[]);
    assert_eq!(number(1_205_000), r#"{"number":"120.5"}"#);
    assert_eq!(number(-30_000), r#"{"number":"-3"}"#);
    assert_eq!(number(0), r#"{"number":"0"}"#);
}

#[test]
fn a_text_is_escaped_not_doubled() {
    let text = scalar(AddressOf(A, Data::Text(0)), Desc::Text, &[b"say \"hi\""]);
    assert_eq!(text, r#"{"text":"say \"hi\""}"#);
}

#[test]
fn a_truth_is_yes_or_no() {
    let truth = |n| scalar(Const(A, n), Desc::Truth, &[]);
    assert_eq!(truth(1), r#"{"truth":"yes"}"#);
    assert_eq!(truth(0), r#"{"truth":"no"}"#);
}

#[test]
fn json_int_writes_a_whole_number_in_decimal() {
    let int = |n| written("json_int", vec![Const(A, n)], &[], Module::default());
    assert_eq!(int(0), "0");
    assert_eq!(int(7), "7");
    assert_eq!(int(1_234_567), "1234567");
    assert_eq!(int(-10), "-10");
}
