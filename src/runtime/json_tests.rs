//! Tests of `json_bytes`: the exact bytes it writes for each kind of input.

use super::tests::{program, run};
use crate::ir::Reg::A;
use crate::ir::{Data, Inst::*};

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
