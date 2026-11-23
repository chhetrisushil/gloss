//! Shared byte emission: REX prefixes and ModRM/SIB/displacement operands.

use super::inst::{Mem, R, Sym};

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

/// The base register of a memory operand, if it has one.
pub fn mem_base(m: &Mem) -> Option<R> {
    match m {
        Mem::Base(r, _) => Some(*r),
        Mem::Rip(_) => None,
    }
}

/// Emits ModRM (reg field `reg`), the SIB byte if needed, and the 32-bit
/// displacement. For `Mem::Rip` the displacement is a zero placeholder; its
/// offset and symbol are returned so the caller can record a fixup.
pub fn modrm_mem(out: &mut Vec<u8>, reg: u8, m: &Mem) -> Option<(usize, Sym)> {
    let reg = (reg & 7) << 3;
    match m {
        Mem::Base(base, disp) => {
            let rm = code(*base) & 7;
            out.push(0b10_000_000 | reg | rm);
            if rm == 4 {
                out.push(0x24);
            }
            out.extend_from_slice(&disp.to_le_bytes());
            None
        }
        Mem::Rip(sym) => {
            out.push(reg | 0b101);
            let at = out.len();
            out.extend_from_slice(&[0; 4]);
            Some((at, sym.clone()))
        }
    }
}

/// Emits ModRM for a register-direct operand.
pub fn modrm_reg(out: &mut Vec<u8>, reg: R, rm: R) {
    out.push(0b11_000_000 | ((code(reg) & 7) << 3) | (code(rm) & 7));
}
