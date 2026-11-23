//! Shared byte emission: REX prefixes and ModRM/SIB/displacement operands.

use super::inst::R;

/// The 4-bit hardware number of a register.
pub fn code(r: R) -> u8 {
    match r {
        R::Rax => 0,
        R::Rcx => 1,
        R::Rdx => 2,
        R::Rbx => 3,
        R::Rsp => 4,
        R::Rbp => 5,
        R::Rsi => 6,
        R::Rdi => 7,
        R::R8 => 8,
        R::R9 => 9,
        R::R10 => 10,
        R::R11 => 11,
        R::R12 => 12,
        R::R13 => 13,
        R::R14 => 14,
        R::R15 => 15,
    }
}

/// Emits a REX prefix. `reg` goes in ModRM.reg (REX.R), `base` in ModRM.rm or
/// the opcode register (REX.B). `force` emits 0x40 even with no bit set, so
/// spl/bpl/sil/dil name the low byte rather than ah..bh.
pub fn rex(out: &mut Vec<u8>, w: bool, reg: R, base: Option<R>, force: bool) {
    let r = code(reg) >> 3;
    let b = base.map_or(0, |b| code(b) >> 3);
    let byte = 0x40 | (u8::from(w) << 3) | (r << 2) | b;
    if byte != 0x40 || force {
        out.push(byte);
    }
}

/// Emits ModRM for a register-direct operand.
pub fn modrm_reg(out: &mut Vec<u8>, reg: R, rm: R) {
    out.push(0b11_000_000 | ((code(reg) & 7) << 3) | (code(rm) & 7));
}
