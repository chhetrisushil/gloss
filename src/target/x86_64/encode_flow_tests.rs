use super::encode::{Encoded, Fixup, encode};
use super::encode_flow::resolve;
use super::inst::{Cc, Mem, R, Sym, X};
use std::collections::HashMap;

fn l(n: &str) -> String {
    n.to_string()
}

fn bytes(xs: &[X]) -> Vec<u8> {
    encode(xs).unwrap().bytes
}

#[test]
fn jumps_to_labels() {
    assert_eq!(
        bytes(&[X::Label(l("L")), X::Jmp(l("L"))]),
        [0xe9, 0xfb, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        bytes(&[X::Jmp(l("L")), X::Label(l("L"))]),
        [0xe9, 0, 0, 0, 0]
    );
    assert_eq!(
        bytes(&[X::Jcc(Cc::O, l("L")), X::Ret, X::Label(l("L"))]),
        [0x0f, 0x80, 1, 0, 0, 0, 0xc3]
    );
}

#[test]
fn calls_reach_functions_before_and_after() {
    let xs = [X::Call(l("f")), X::Ret, X::Label(l("f")), X::Ret];
    assert_eq!(bytes(&xs), [0xe8, 1, 0, 0, 0, 0xc3, 0xc3]);
    let xs = [X::Label(l("f")), X::Ret, X::Call(l("f"))];
    assert_eq!(bytes(&xs), [0xc3, 0xe8, 0xfa, 0xff, 0xff, 0xff]);
}

#[test]
fn every_condition_has_its_opcode() {
    let all = [
        (Cc::L, 0x8c),
        (Cc::Ge, 0x8d),
        (Cc::B, 0x82),
        (Cc::Ae, 0x83),
        (Cc::E, 0x84),
        (Cc::Ne, 0x85),
        (Cc::Le, 0x8e),
        (Cc::G, 0x8f),
    ];
    for (cc, op) in all {
        let b = bytes(&[X::Label(l("L")), X::Jcc(cc, l("L"))]);
        assert_eq!(b, [0x0f, op, 0xfa, 0xff, 0xff, 0xff]);
    }
}

#[test]
fn labels_record_offsets_and_comments_emit_nothing() {
    let e = encode(&[
        X::Comment(l("hi")),
        X::Label(l("a")),
        X::Ret,
        X::Label(l("b")),
    ])
    .unwrap();
    assert_eq!(e.bytes, [0xc3]);
    assert_eq!((e.labels["a"], e.labels["b"]), (0, 1));
    assert!(e.fixups.is_empty());
}

#[test]
fn rip_references_to_labels_are_resolved_and_removed() {
    let xs = [
        X::Lea(R::Rax, Mem::Rip(Sym::Label(l("L")))),
        X::Ret,
        X::Label(l("L")),
    ];
    let e = encode(&xs).unwrap();
    assert_eq!(e.bytes, [0x48, 0x8d, 0x05, 1, 0, 0, 0, 0xc3]);
    assert!(e.fixups.is_empty());
}

#[test]
fn data_fixups_survive_with_their_offsets() {
    let xs = [
        X::Jmp(l("L")),
        X::Lea(R::Rax, Mem::Rip(Sym::Text(2))),
        X::Lea(R::Rcx, Mem::Rip(Sym::Desc(5))),
        X::Label(l("L")),
    ];
    let e = encode(&xs).unwrap();
    let got: Vec<_> = e.fixups.iter().map(|f| (f.at, f.end)).collect();
    assert_eq!(got, [(8, 12), (15, 19)]);
    assert_eq!(e.fixups[0].target, Sym::Text(2));
    assert_eq!(e.fixups[1].target, Sym::Desc(5));
}

#[test]
fn backward_rip_label_is_patched_and_removed() {
    let xs = [
        X::Label(l("L")),
        X::Ret,
        X::Ret,
        X::Lea(R::Rax, Mem::Rip(Sym::Label(l("L")))),
    ];
    let e = encode(&xs).unwrap();
    assert_eq!(e.bytes[2..], [0x48, 0x8d, 0x05, 0xf7, 0xff, 0xff, 0xff]);
    assert!(e.fixups.is_empty());
}

#[test]
fn call_ten_bytes_ahead() {
    let xs = [
        X::Call(l("f")),
        X::Syscall,
        X::Syscall,
        X::Ret,
        X::Label(l("f")),
    ];
    let b = bytes(&xs);
    assert_eq!(b[..5], [0xe8, 0x05, 0, 0, 0]);
    assert_eq!(b.len(), 10);
}

fn hand(to: usize, end: usize) -> Encoded {
    Encoded {
        bytes: vec![0; 4],
        labels: HashMap::from([(l("L"), to)]),
        fixups: vec![Fixup {
            at: 0,
            target: Sym::Label(l("L")),
            end,
        }],
    }
}

#[test]
fn rel32_reach_is_exact() {
    let mut e = hand(4 + i32::MAX as usize, 4);
    resolve(&mut e).unwrap();
    assert_eq!(e.bytes, i32::MAX.to_le_bytes());
    assert!(e.fixups.is_empty());
    let mut e = hand(0, 1 << 31);
    resolve(&mut e).unwrap();
    assert_eq!(e.bytes, i32::MIN.to_le_bytes());
    let far = "label L is too far away";
    assert_eq!(
        resolve(&mut hand(5 + i32::MAX as usize, 4)).unwrap_err(),
        far
    );
    assert_eq!(resolve(&mut hand(0, (1 << 31) + 1)).unwrap_err(), far);
}

#[test]
fn label_errors() {
    assert_eq!(encode(&[X::Jmp(l("x"))]).unwrap_err(), "unknown label x");
    assert_eq!(
        encode(&[X::Label(l("x")), X::Label(l("x"))]).unwrap_err(),
        "label x defined twice"
    );
}

#[test]
fn long_distances() {
    let mut xs = vec![X::Jmp(l("L"))];
    xs.extend(std::iter::repeat_n(X::Ret, 70_000));
    xs.push(X::Label(l("L")));
    let b = bytes(&xs);
    assert_eq!(b[..5], [0xe9, 0x70, 0x11, 0x01, 0]);
}
