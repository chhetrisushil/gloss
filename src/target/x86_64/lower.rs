//! Lowers Gloss IR functions to x86-64 instructions.
//!
//! `a` is `rax` and `b` is `rcx`. Slot `k` is `[rbp-8k]`; the frame has one more slot, below
//! the last, where `Enter` saves `b` and `Return` restores it, so `b` survives every call.
//! Scratch registers (`rdx`, `rsi`, `rdi`, `r8`-`r11`) live only inside one instruction.

use std::collections::HashMap;

use super::inst::{AluOp, Mem, R, X};
use crate::ir::{Function, Inst, Reg, inst_text};

/// Lowers one function; `params` maps every callable function to its parameter count.
pub fn lower_function(f: &Function, params: &HashMap<String, u32>) -> Result<Vec<X>, String> {
    let mut out = vec![X::Label(f.name.clone())];
    for inst in &f.body {
        inst_x(f, params, inst, &mut out)?;
    }
    Ok(out)
}

fn reg(r: Reg) -> R {
    match r {
        Reg::A => R::Rax,
        Reg::B => R::Rcx,
    }
}
fn slot(k: u32) -> Mem {
    Mem::Base(R::Rbp, -8 * k as i32)
}

fn arg(n: u32, k: u32) -> Mem {
    Mem::Base(R::Rbp, 16 + 8 * (n - k) as i32)
}

fn inst_x(
    f: &Function,
    _params: &HashMap<String, u32>,
    inst: &Inst,
    out: &mut Vec<X>,
) -> Result<(), String> {
    match inst {
        Inst::Line(n) => out.push(X::Comment(format!("line {n}"))),
        Inst::LoadSlot(r, k) => out.push(X::MovRM(reg(*r), slot(*k))),
        Inst::StoreSlot(k, r) => out.push(X::MovMR(slot(*k), reg(*r))),
        Inst::LoadArg(r, k) => out.push(X::MovRM(reg(*r), arg(f.params, *k))),
        Inst::Return => {
            out.push(X::MovRM(R::Rcx, slot(f.slots + 1)));
            out.push(X::MovRR(R::Rsp, R::Rbp));
            out.push(X::Pop(R::Rbp));
            out.push(X::Ret);
        }
        Inst::Enter(n) => enter(*n, out),
        Inst::Const(..)
        | Inst::Move { .. }
        | Inst::Push(_)
        | Inst::Pop(_)
        | Inst::AddressOf(..)
        | Inst::StackPointer(_)
        | Inst::Load { .. }
        | Inst::Store { .. }
        | Inst::LoadByte { .. }
        | Inst::StoreByte { .. }
        | Inst::Int(_)
        | Inst::AddImm(..)
        | Inst::Shift(..)
        | Inst::Compare
        | Inst::JumpIf(..)
        | Inst::SetIf(_)
        | Inst::Jump(_)
        | Inst::Mark(_)
        | Inst::Call(_)
        | Inst::StackCheck { .. }
        | Inst::NumAdd { .. }
        | Inst::NumSub { .. }
        | Inst::NumNeg { .. }
        | Inst::NumMul { .. }
        | Inst::NumDiv { .. }
        | Inst::NumRem { .. }
        | Inst::NumWhole
        | Inst::System(_) => {
            return Err(format!("cannot lower {} yet", inst_text(inst).trim()));
        }
    }
    Ok(())
}

/// Saves `b` in the slot below the last, then zeroes slots 1..=n.
fn enter(n: u32, out: &mut Vec<X>) {
    let bytes = 8 * (n + 1) as i32;
    out.push(X::Push(R::Rbp));
    out.push(X::MovRR(R::Rbp, R::Rsp));
    out.push(X::AluI(AluOp::Sub, R::Rsp, bytes));
    out.push(X::MovMR(slot(n + 1), R::Rcx));
    out.push(X::Alu(AluOp::Xor, R::Rdx, R::Rdx));
    for k in 1..=n {
        out.push(X::MovMR(slot(k), R::Rdx));
    }
}
