use super::encode::{Fixup, encode};
use super::inst::{Mem, R, Sym, X};

fn bytes(x: X) -> Vec<u8> {
    encode(&[x]).unwrap().bytes
}

fn at(r: R, d: i32) -> Mem {
    Mem::Base(r, d)
}

#[test]
fn register_moves_and_immediates() {
    assert_eq!(bytes(X::MovRR(R::Rax, R::Rcx)), [0x48, 0x89, 0xc8]);
    assert_eq!(bytes(X::MovRR(R::R12, R::Rax)), [0x49, 0x89, 0xc4]);
    assert_eq!(bytes(X::MovRR(R::Rax, R::R9)), [0x4c, 0x89, 0xc8]);
    assert_eq!(
        bytes(X::MovRI(R::Rax, 42)),
        [0x48, 0xb8, 42, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovRI(R::R9, -1)),
        [0x49, 0xb9, 255, 255, 255, 255, 255, 255, 255, 255]
    );
}

#[test]
fn memory_moves_use_disp32() {
    let rbp = [0x48, 0x8b, 0x85, 0xf0, 0xff, 0xff, 0xff];
    assert_eq!(bytes(X::MovRM(R::Rax, at(R::Rbp, -16))), rbp);
    assert_eq!(
        bytes(X::MovMR(at(R::Rbp, -16), R::Rax)),
        [0x48, 0x89, 0x85, 0xf0, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        bytes(X::MovRM(R::Rax, at(R::Rsp, 8))),
        [0x48, 0x8b, 0x84, 0x24, 8, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovRM(R::Rax, at(R::R12, 0))),
        [0x49, 0x8b, 0x84, 0x24, 0, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovRM(R::Rcx, at(R::R13, 8))),
        [0x49, 0x8b, 0x8d, 8, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovRM(R::R10, at(R::Rcx, 8))),
        [0x4c, 0x8b, 0x91, 8, 0, 0, 0]
    );
}

#[test]
fn byte_moves() {
    assert_eq!(
        bytes(X::MovzxRM8(R::Rax, at(R::Rcx, 8))),
        [0x0f, 0xb6, 0x81, 8, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovzxRM8(R::R10, at(R::R9, 8))),
        [0x45, 0x0f, 0xb6, 0x91, 8, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovM8R(at(R::Rcx, 8), R::Rax)),
        [0x88, 0x81, 8, 0, 0, 0]
    );
    assert_eq!(
        bytes(X::MovM8R(at(R::Rax, 0), R::Rsi)),
        [0x40, 0x88, 0xb0, 0, 0, 0, 0]
    );
    for (r, modrm) in [(R::Rsp, 0xa0), (R::Rbp, 0xa8), (R::Rdi, 0xb8)] {
        assert_eq!(
            bytes(X::MovM8R(at(R::Rax, 0), r)),
            [0x40, 0x88, modrm, 0, 0, 0, 0]
        );
    }
    assert_eq!(
        bytes(X::MovM8R(at(R::R8, 0), R::R12)),
        [0x45, 0x88, 0xa0, 0, 0, 0, 0]
    );
}

#[test]
fn rip_relative_records_a_fixup() {
    let e = encode(&[X::Lea(R::Rax, Mem::Rip(Sym::Text(0)))]).unwrap();
    assert_eq!(e.bytes, [0x48, 0x8d, 0x05, 0, 0, 0, 0]);
    assert_eq!((e.fixups[0].at, e.fixups[0].end), (3, 7));
    let sym = Sym::Text(3);
    let e = encode(&[X::Push(R::Rax), X::Lea(R::Rax, Mem::Rip(sym.clone()))]).unwrap();
    assert_eq!(e.bytes, [0x50, 0x48, 0x8d, 0x05, 0, 0, 0, 0]);
    assert_eq!(
        e.fixups,
        [Fixup {
            at: 4,
            target: sym,
            end: 8
        }]
    );
    let e = encode(&[X::MovRM(R::R9, Mem::Rip(Sym::Desc(1)))]).unwrap();
    assert_eq!(e.bytes, [0x4c, 0x8b, 0x0d, 0, 0, 0, 0]);
    assert_eq!((e.fixups[0].at, e.fixups[0].end), (3, 7));
}

#[test]
fn push_and_pop() {
    assert_eq!(bytes(X::Push(R::Rbp)), [0x55]);
    assert_eq!(bytes(X::Pop(R::Rbp)), [0x5d]);
    assert_eq!(bytes(X::Push(R::R12)), [0x41, 0x54]);
    assert_eq!(bytes(X::Pop(R::R15)), [0x41, 0x5f]);
}

#[test]
fn unfinished_instructions_are_errors() {
    assert_eq!(encode(&[X::Ret]).unwrap_err(), "cannot encode ret yet");
}
