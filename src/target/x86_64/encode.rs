//! Encodes typed x86-64 instructions to machine code.

use std::collections::HashMap;

use super::emit::{code, mem_base, modrm_mem, modrm_reg, rex};
use super::encode_alu as alu;
use super::inst::{Mem, R, Sym, X, text};

/// A 32-bit field at `at`, relative to `end`, the end of its instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixup {
    pub at: usize,
    pub target: Sym,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub labels: HashMap<String, usize>,
    pub fixups: Vec<Fixup>,
}

pub fn encode(xs: &[X]) -> Result<Encoded, String> {
    let mut e = Encoded {
        bytes: Vec::new(),
        labels: HashMap::new(),
        fixups: Vec::new(),
    };
    for x in xs {
        one(&mut e, x)?;
    }
    Ok(e)
}

/// `op /r` with a register and a memory operand; `w` selects REX.W.
fn reg_mem(e: &mut Encoded, w: bool, op: &[u8], r: R, m: &Mem, force: bool) {
    rex(&mut e.bytes, w, r, mem_base(m), force);
    e.bytes.extend_from_slice(op);
    if let Some((at, target)) = modrm_mem(&mut e.bytes, code(r), m) {
        e.fixups.push(Fixup {
            at,
            target,
            end: e.bytes.len(),
        });
    }
}

fn one(e: &mut Encoded, x: &X) -> Result<(), String> {
    match x {
        X::MovRR(d, s) => {
            rex(&mut e.bytes, true, *s, Some(*d), false);
            e.bytes.push(0x89);
            modrm_reg(&mut e.bytes, *s, *d);
        }
        X::MovRI(d, n) => {
            rex(&mut e.bytes, true, R::Rax, Some(*d), false);
            e.bytes.push(0xb8 | (code(*d) & 7));
            e.bytes.extend_from_slice(&n.to_le_bytes());
        }
        X::MovRM(d, m) => reg_mem(e, true, &[0x8b], *d, m, false),
        X::MovMR(m, s) => reg_mem(e, true, &[0x89], *s, m, false),
        X::MovzxRM8(d, m) => reg_mem(e, false, &[0x0f, 0xb6], *d, m, false),
        X::MovM8R(m, s) => {
            let low = matches!(s, R::Rsp | R::Rbp | R::Rsi | R::Rdi);
            reg_mem(e, false, &[0x88], *s, m, low);
        }
        X::Lea(d, m) => reg_mem(e, true, &[0x8d], *d, m, false),
        X::Push(r) => {
            rex(&mut e.bytes, false, R::Rax, Some(*r), false);
            e.bytes.push(0x50 | (code(*r) & 7));
        }
        X::Pop(r) => {
            rex(&mut e.bytes, false, R::Rax, Some(*r), false);
            e.bytes.push(0x58 | (code(*r) & 7));
        }
        X::Alu(op, d, s) => alu::rr(&mut e.bytes, &[alu::alu_opcode(*op)], *s, *d),
        X::AluI(op, d, n) => {
            alu::digit(
                &mut e.bytes,
                0x81,
                alu::alu_digit(*op),
                *d,
                &n.to_le_bytes(),
            );
        }
        X::Imul(d, s) => alu::rr(&mut e.bytes, &[0x0f, 0xaf], *d, *s),
        X::ImulWide(r) => alu::digit(&mut e.bytes, 0xf7, 5, *r, &[]),
        X::Mul(r) => alu::digit(&mut e.bytes, 0xf7, 4, *r, &[]),
        X::Idiv(r) => alu::digit(&mut e.bytes, 0xf7, 7, *r, &[]),
        X::Div(r) => alu::digit(&mut e.bytes, 0xf7, 6, *r, &[]),
        X::Cqo => e.bytes.extend_from_slice(&[0x48, 0x99]),
        X::Neg(r) => alu::digit(&mut e.bytes, 0xf7, 3, *r, &[]),
        X::Test(a, b) => alu::rr(&mut e.bytes, &[0x85], *b, *a),
        X::Shl(r, n) => alu::digit(&mut e.bytes, 0xc1, 4, *r, &[*n]),
        X::Sar(r, n) => alu::digit(&mut e.bytes, 0xc1, 7, *r, &[*n]),
        X::Shr(r, n) => alu::digit(&mut e.bytes, 0xc1, 5, *r, &[*n]),
        X::Setcc(c, r) => alu::setcc(&mut e.bytes, *c, *r),
        X::Ret => e.bytes.push(0xc3),
        X::Syscall => e.bytes.extend_from_slice(&[0x0f, 0x05]),
        X::Jcc(..) | X::Jmp(_) | X::Call(_) | X::Label(_) | X::Comment(_) => {
            return Err(format!("cannot encode {} yet", text(x)));
        }
    }
    Ok(())
}
