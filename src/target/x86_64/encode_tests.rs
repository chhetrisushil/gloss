use super::encode::{Fixup, encode};
use super::inst::{AluOp, Cc, Mem, R, Sym, X};

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
    assert_eq!(
        encode(&[X::Jmp("l".into())]).unwrap_err(),
        "cannot encode jmp l yet"
    );
}

#[test]
fn register_arithmetic() {
    let alu = |op| bytes(X::Alu(op, R::Rax, R::Rcx));
    assert_eq!(alu(AluOp::Add), [0x48, 0x01, 0xc8]);
    assert_eq!(alu(AluOp::Sub), [0x48, 0x29, 0xc8]);
    assert_eq!(alu(AluOp::Cmp), [0x48, 0x39, 0xc8]);
    assert_eq!(alu(AluOp::And), [0x48, 0x21, 0xc8]);
    assert_eq!(alu(AluOp::Or), [0x48, 0x09, 0xc8]);
    assert_eq!(
        bytes(X::Alu(AluOp::Xor, R::Rdx, R::Rdx)),
        [0x48, 0x31, 0xd2]
    );
    assert_eq!(bytes(X::Alu(AluOp::Add, R::R9, R::Rax)), [0x49, 0x01, 0xc1]);
    assert_eq!(bytes(X::Alu(AluOp::Add, R::Rax, R::R9)), [0x4c, 0x01, 0xc8]);
    assert_eq!(bytes(X::Imul(R::Rax, R::Rcx)), [0x48, 0x0f, 0xaf, 0xc1]);
    assert_eq!(bytes(X::Imul(R::R8, R::R9)), [0x4d, 0x0f, 0xaf, 0xc1]);
    assert_eq!(bytes(X::Test(R::Rax, R::Rax)), [0x48, 0x85, 0xc0]);
}

#[test]
fn immediate_arithmetic_is_always_imm32() {
    let sub = bytes(X::AluI(AluOp::Sub, R::Rsp, 24));
    assert_eq!(sub, [0x48, 0x81, 0xec, 24, 0, 0, 0]);
    let add = bytes(X::AluI(AluOp::Add, R::Rax, -8));
    assert_eq!(add, [0x48, 0x81, 0xc0, 0xf8, 0xff, 0xff, 0xff]);
    let cmp = bytes(X::AluI(AluOp::Cmp, R::R10, 1));
    assert_eq!(cmp, [0x49, 0x81, 0xfa, 1, 0, 0, 0]);
    let digits = [
        (AluOp::Add, 0xc0),
        (AluOp::Or, 0xc8),
        (AluOp::And, 0xe0),
        (AluOp::Sub, 0xe8),
        (AluOp::Xor, 0xf0),
        (AluOp::Cmp, 0xf8),
    ];
    for (op, modrm) in digits {
        assert_eq!(bytes(X::AluI(op, R::Rax, 0))[2], modrm);
    }
}

#[test]
fn unary_and_shifts() {
    assert_eq!(bytes(X::ImulWide(R::Rcx)), [0x48, 0xf7, 0xe9]);
    assert_eq!(bytes(X::Mul(R::Rcx)), [0x48, 0xf7, 0xe1]);
    assert_eq!(bytes(X::Idiv(R::Rcx)), [0x48, 0xf7, 0xf9]);
    assert_eq!(bytes(X::Div(R::Rcx)), [0x48, 0xf7, 0xf1]);
    assert_eq!(bytes(X::Neg(R::Rax)), [0x48, 0xf7, 0xd8]);
    assert_eq!(bytes(X::Neg(R::R11)), [0x49, 0xf7, 0xdb]);
    assert_eq!(bytes(X::Cqo), [0x48, 0x99]);
    assert_eq!(bytes(X::Shl(R::Rax, 3)), [0x48, 0xc1, 0xe0, 3]);
    assert_eq!(bytes(X::Sar(R::Rax, 3)), [0x48, 0xc1, 0xf8, 3]);
    assert_eq!(bytes(X::Shr(R::Rax, 3)), [0x48, 0xc1, 0xe8, 3]);
    assert_eq!(bytes(X::Shl(R::R12, 63)), [0x49, 0xc1, 0xe4, 63]);
}

#[test]
fn setcc_syscall_ret() {
    assert_eq!(bytes(X::Setcc(Cc::L, R::Rax)), [0x0f, 0x9c, 0xc0]);
    assert_eq!(bytes(X::Setcc(Cc::E, R::Rsi)), [0x40, 0x0f, 0x94, 0xc6]);
    assert_eq!(bytes(X::Setcc(Cc::G, R::R9)), [0x41, 0x0f, 0x9f, 0xc1]);
    assert_eq!(bytes(X::Setcc(Cc::E, R::Rsp)), [0x40, 0x0f, 0x94, 0xc4]);
    assert_eq!(bytes(X::Setcc(Cc::E, R::Rbp)), [0x40, 0x0f, 0x94, 0xc5]);
    assert_eq!(bytes(X::Setcc(Cc::E, R::Rdi)), [0x40, 0x0f, 0x94, 0xc7]);
    assert_eq!(bytes(X::Setcc(Cc::E, R::R15)), [0x41, 0x0f, 0x94, 0xc7]);
    let nibbles = [
        (Cc::O, 0x90),
        (Cc::B, 0x92),
        (Cc::Ae, 0x93),
        (Cc::E, 0x94),
        (Cc::Ne, 0x95),
        (Cc::Le, 0x9e),
        (Cc::L, 0x9c),
        (Cc::Ge, 0x9d),
        (Cc::G, 0x9f),
    ];
    for (cc, op) in nibbles {
        assert_eq!(bytes(X::Setcc(cc, R::Rax))[1], op);
    }
    assert_eq!(bytes(X::Syscall), [0x0f, 0x05]);
    assert_eq!(bytes(X::Ret), [0xc3]);
}
