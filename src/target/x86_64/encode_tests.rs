use super::encode::encode;
use super::inst::{R, X};

fn bytes(x: X) -> Vec<u8> {
    encode(&[x]).unwrap().bytes
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
