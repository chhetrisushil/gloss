//! Byte layouts for the arithmetic, shift and flag instructions.

use super::emit::{code, modrm_reg, rex};
use super::inst::{AluOp, Cc, R};

/// `REX.W op /r` on two registers: `reg` in ModRM.reg, `rm` in ModRM.rm.
pub fn rr(out: &mut Vec<u8>, op: &[u8], reg: R, rm: R) {
    rex(out, true, reg, Some(rm), false);
    out.extend_from_slice(op);
    modrm_reg(out, reg, rm);
}

/// `REX.W op /ext` on one register, followed by `tail`.
pub fn digit(out: &mut Vec<u8>, op: u8, ext: u8, rm: R, tail: &[u8]) {
    rex(out, true, R::Rax, Some(rm), false);
    out.push(op);
    out.push(0b11_000_000 | (ext << 3) | (code(rm) & 7));
    out.extend_from_slice(tail);
}

/// The `op r/m64, r64` opcode of an ALU operation.
pub fn alu_opcode(op: AluOp) -> u8 {
    match op {
        AluOp::Add => 0x01,
        AluOp::Or => 0x09,
        AluOp::And => 0x21,
        AluOp::Sub => 0x29,
        AluOp::Xor => 0x31,
        AluOp::Cmp => 0x39,
    }
}

/// The `/digit` of an ALU operation in the immediate group (opcode 0x81).
pub fn alu_digit(op: AluOp) -> u8 {
    match op {
        AluOp::Add => 0,
        AluOp::Or => 1,
        AluOp::And => 4,
        AluOp::Sub => 5,
        AluOp::Xor => 6,
        AluOp::Cmp => 7,
    }
}

/// `setcc r8`: `0F 9x /0`, with REX for spl/bpl/sil/dil and r8..r15.
pub fn setcc(out: &mut Vec<u8>, c: Cc, r: R) {
    let low = matches!(r, R::Rsp | R::Rbp | R::Rsi | R::Rdi);
    rex(out, false, R::Rax, Some(r), low);
    out.extend_from_slice(&[0x0f, 0x90 | cc_nibble(c), 0b11_000_000 | (code(r) & 7)]);
}

pub fn cc_nibble(c: Cc) -> u8 {
    match c {
        Cc::O => 0x0,
        Cc::B => 0x2,
        Cc::Ae => 0x3,
        Cc::E => 0x4,
        Cc::Ne => 0x5,
        Cc::Le => 0xe,
        Cc::L => 0xc,
        Cc::Ge => 0xd,
        Cc::G => 0xf,
    }
}
