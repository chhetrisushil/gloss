//! Lowers Gloss IR functions to x86-64 instructions.
//!
//! `a` is `rax` and `b` is `rcx`. Slot `k` is `[rbp-8k]`; the frame has one more slot, below
//! the last, where `Enter` saves `b` and `Return` restores it, so `b` survives every call.
//! Scratch registers (`rdx`, `rsi`, `rdi`, `r8`-`r11`) live only inside one instruction.

use std::collections::HashMap;

use super::inst::{AluOp, Cc, Mem, R, Sym, X};
use crate::ir::{Cond, Data, Function, Global, Inst, Label, Reg, inst_text};

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
fn label(f: &Function, l: Label) -> String {
    format!("L{}_{}", f.name, l.0)
}

fn cc(c: Cond) -> Cc {
    match c {
        Cond::Eq => Cc::E,
        Cond::Ne => Cc::Ne,
        Cond::Lt => Cc::L,
        Cond::Le => Cc::Le,
        Cond::Gt => Cc::G,
        Cond::Ge => Cc::Ge,
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
    params: &HashMap<String, u32>,
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
        Inst::Const(r, v) => out.push(X::MovRI(reg(*r), *v)),
        Inst::Move { to, from } => out.push(X::MovRR(reg(*to), reg(*from))),
        Inst::Push(r) => out.push(X::Push(reg(*r))),
        Inst::Pop(r) => out.push(X::Pop(reg(*r))),
        Inst::AddressOf(r, d) => {
            let sym = match d {
                Data::Text(n) => Sym::Text(*n),
                Data::Desc(n) => Sym::Desc(*n),
                Data::Global(g) => Sym::Global(*g),
            };
            out.push(X::Lea(reg(*r), Mem::Rip(sym)));
        }
        Inst::StackPointer(r) => out.push(X::MovRR(reg(*r), R::Rsp)),
        Inst::Load { to, base, offset } => {
            out.push(X::MovRM(reg(*to), Mem::Base(reg(*base), *offset)));
        }
        Inst::Store { base, offset, from } => {
            out.push(X::MovMR(Mem::Base(reg(*base), *offset), reg(*from)));
        }
        Inst::LoadByte { to, base, offset } => {
            out.push(X::MovzxRM8(reg(*to), Mem::Base(reg(*base), *offset)));
        }
        Inst::StoreByte { base, offset, from } => {
            out.push(X::MovM8R(Mem::Base(reg(*base), *offset), reg(*from)));
        }
        Inst::Compare => out.push(X::Alu(AluOp::Cmp, R::Rax, R::Rcx)),
        Inst::JumpIf(c, l) => out.push(X::Jcc(cc(*c), label(f, *l))),
        Inst::SetIf(c) => {
            out.push(X::Setcc(cc(*c), R::Rax));
            out.push(X::AluI(AluOp::And, R::Rax, 255));
        }
        Inst::Jump(l) => out.push(X::Jmp(label(f, *l))),
        Inst::Mark(l) => out.push(X::Label(label(f, *l))),
        Inst::Call(name) => {
            let Some(p) = params.get(name) else {
                return Err(format!("call to unknown function {name}"));
            };
            out.push(X::Call(name.clone()));
            if *p > 0 {
                out.push(X::AluI(AluOp::Add, R::Rsp, 8 * *p as i32));
            }
        }
        Inst::StackCheck { bytes, fail } => {
            out.push(X::Lea(R::Rdx, Mem::Base(R::Rsp, -(*bytes as i32))));
            out.push(X::MovRM(R::Rsi, Mem::Rip(Sym::Global(Global::StackLimit))));
            out.push(X::Alu(AluOp::Cmp, R::Rdx, R::Rsi));
            out.push(X::Jcc(Cc::B, label(f, *fail)));
        }
        Inst::Int(_)
        | Inst::AddImm(..)
        | Inst::Shift(..)
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
