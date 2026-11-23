//! Encodes typed x86-64 instructions to machine code.

use std::collections::HashMap;

use super::emit::{code, modrm_reg, rex};
use super::inst::{R, Sym, X, text};

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
        X::Push(r) => {
            rex(&mut e.bytes, false, R::Rax, Some(*r), false);
            e.bytes.push(0x50 | (code(*r) & 7));
        }
        X::Pop(r) => {
            rex(&mut e.bytes, false, R::Rax, Some(*r), false);
            e.bytes.push(0x58 | (code(*r) & 7));
        }
        X::MovRM(..)
        | X::MovMR(..)
        | X::MovzxRM8(..)
        | X::MovM8R(..)
        | X::Lea(..)
        | X::Alu(..)
        | X::AluI(..)
        | X::Imul(..)
        | X::ImulWide(_)
        | X::Mul(_)
        | X::Idiv(_)
        | X::Div(_)
        | X::Cqo
        | X::Neg(_)
        | X::Test(..)
        | X::Shl(..)
        | X::Sar(..)
        | X::Shr(..)
        | X::Setcc(..)
        | X::Jcc(..)
        | X::Jmp(_)
        | X::Call(_)
        | X::Ret
        | X::Syscall
        | X::Label(_)
        | X::Comment(_) => return Err(format!("cannot encode {} yet", text(x))),
    }
    Ok(())
}
