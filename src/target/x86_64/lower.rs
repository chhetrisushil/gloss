//! Lowers Gloss IR functions to x86-64 instructions.
//!
//! `a` is `rax` and `b` is `rcx`. Slot `k` is `[rbp-8k]`; the frame has one more slot, below
//! the last, where `Enter` saves `b` and `Return` restores it, so `b` survives every call.
//! Scratch registers (`rdx`, `rsi`, `rdi`, `r8`-`r11`) live only inside one instruction.

use std::collections::HashMap;

use super::arith;
use super::inst::{AluOp, Cc, Mem, R, Sym, X};
use super::linux;
use crate::ir::{Cond, Data, Function, Global, Inst, IntOp, Label, Reg, Shift};
use crate::number::SCALE;

/// Lowers one function; `params` maps every callable function to its parameter count.
/// `Int(Div)` and `Int(Rem)` require a divisor greater than 0 (ruling M2-R7): `idiv` is
/// only defined for that range here and no check is emitted.
pub fn lower_function(f: &Function, params: &HashMap<String, u32>) -> Result<Vec<X>, String> {
    let mut out = vec![X::Label(f.name.clone())];
    if f.name == "_start" {
        // The kernel enters `_start` with `rsp` at `argc`; keep it for the stack limit.
        out.push(X::MovMR(Mem::Rip(Sym::Global(Global::EntrySp)), R::Rsp));
    }
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
/// A byte count or offset as an `i32` displacement or immediate, or an error.
fn disp(bytes: u64) -> Result<i32, String> {
    i32::try_from(bytes).map_err(|_| format!("{bytes} bytes is out of range for the x86 lowering"))
}

/// The word `k` below `rbp`, with no check that `k` names a slot.
fn word(k: u64) -> Result<Mem, String> {
    Ok(Mem::Base(R::Rbp, -disp(8 * k)?))
}

/// Slot `k`, which must be in `1..=f.slots`, so the saved `b` word is never aliased.
fn slot(f: &Function, k: u32) -> Result<Mem, String> {
    if !(1..=f.slots).contains(&k) {
        return Err(format!("slot {k} is outside 1..={} in {}", f.slots, f.name));
    }
    word(u64::from(k))
}

/// Argument `k` of the function, which must be in `1..=f.params`.
fn arg(f: &Function, k: u32) -> Result<Mem, String> {
    if !(1..=f.params).contains(&k) {
        return Err(format!(
            "argument {k} is outside 1..={} in {}",
            f.params, f.name
        ));
    }
    let above = 8 * u64::from(f.params - k);
    Ok(Mem::Base(R::Rbp, disp(16 + above)?))
}

fn inst_x(
    f: &Function,
    params: &HashMap<String, u32>,
    inst: &Inst,
    out: &mut Vec<X>,
) -> Result<(), String> {
    match inst {
        Inst::Line(n) => out.push(X::Comment(format!("line {n}"))),
        Inst::LoadSlot(r, k) => out.push(X::MovRM(reg(*r), slot(f, *k)?)),
        Inst::StoreSlot(k, r) => out.push(X::MovMR(slot(f, *k)?, reg(*r))),
        Inst::LoadArg(r, k) => out.push(X::MovRM(reg(*r), arg(f, *k)?)),
        Inst::Return => {
            out.push(X::MovRM(R::Rcx, word(u64::from(f.slots) + 1)?));
            out.push(X::MovRR(R::Rsp, R::Rbp));
            out.push(X::Pop(R::Rbp));
            out.push(X::Ret);
        }
        Inst::Enter(n) => enter(f, *n, out)?,
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
        Inst::Int(op) => int_op(*op, out),
        Inst::AddImm(r, n) => out.push(X::AluI(AluOp::Add, reg(*r), *n)),
        Inst::Shift(s, n) => {
            debug_assert!(*n < 64, "shift count {n} out of range");
            out.push(match s {
                Shift::Left => X::Shl(R::Rax, *n),
                Shift::Right => X::Sar(R::Rax, *n),
                Shift::RightLogical => X::Shr(R::Rax, *n),
            });
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
                out.push(X::AluI(AluOp::Add, R::Rsp, disp(8 * u64::from(*p))?));
            }
        }
        Inst::StackCheck { bytes, fail } => {
            // The number helpers (a return address) and the syscall wrappers (`rcx` and, for
            // StackSize, a 16-byte buffer) use up to 24 bytes below the IR's stack; the
            // plan's `8 * (slots + 3) + 64` margin per frame covers them.
            out.push(X::Lea(R::Rdx, Mem::Base(R::Rsp, -disp(u64::from(*bytes))?)));
            out.push(X::MovRM(R::Rsi, Mem::Rip(Sym::Global(Global::StackLimit))));
            out.push(X::Alu(AluOp::Cmp, R::Rdx, R::Rsi));
            out.push(X::Jcc(Cc::B, label(f, *fail)));
        }
        Inst::NumAdd { range } => checked(AluOp::Add, f, *range, out),
        Inst::NumSub { range } => checked(AluOp::Sub, f, *range, out),
        Inst::NumNeg { range } => out.extend([X::Neg(R::Rax), X::Jcc(Cc::O, label(f, *range))]),
        Inst::NumWhole => out.extend([
            X::Cqo,
            X::MovRI(R::Rsi, SCALE),
            X::Idiv(R::Rsi),
            X::Imul(R::Rax, R::Rsi),
        ]),
        Inst::System(p) => linux::system(
            *p,
            &format!("{}{}_s{}", arith::PREFIX, f.name, out.len()),
            out,
        ),
        Inst::NumMul { range } => {
            out.extend([
                X::Call(arith::MULTIPLY.to_string()),
                X::Test(R::Rdx, R::Rdx),
            ]);
            out.push(X::Jcc(Cc::Ne, label(f, *range)));
        }
        Inst::NumDiv { zero, range } => {
            out.extend([
                X::Call(arith::DIVIDE.to_string()),
                X::AluI(AluOp::Cmp, R::Rdx, 2),
            ]);
            out.push(X::Jcc(Cc::E, label(f, *zero)));
            out.extend([X::Test(R::Rdx, R::Rdx), X::Jcc(Cc::Ne, label(f, *range))]);
        }
        Inst::NumRem { zero } => {
            out.extend([
                X::Call(arith::REMAINDER.to_string()),
                X::Test(R::Rdx, R::Rdx),
            ]);
            out.push(X::Jcc(Cc::Ne, label(f, *zero)));
        }
    }
    Ok(())
}

/// `a = a op b` that jumps to `range` when the result overflows.
fn checked(op: AluOp, f: &Function, range: Label, out: &mut Vec<X>) {
    out.push(X::Alu(op, R::Rax, R::Rcx));
    out.push(X::Jcc(Cc::O, label(f, range)));
}

fn int_op(op: IntOp, out: &mut Vec<X>) {
    match op {
        IntOp::Add => out.push(X::Alu(AluOp::Add, R::Rax, R::Rcx)),
        IntOp::Sub => out.push(X::Alu(AluOp::Sub, R::Rax, R::Rcx)),
        IntOp::And => out.push(X::Alu(AluOp::And, R::Rax, R::Rcx)),
        IntOp::Or => out.push(X::Alu(AluOp::Or, R::Rax, R::Rcx)),
        IntOp::Mul => out.push(X::Imul(R::Rax, R::Rcx)),
        IntOp::Div => out.extend([X::Cqo, X::Idiv(R::Rcx)]),
        IntOp::Rem => out.extend([X::Cqo, X::Idiv(R::Rcx), X::MovRR(R::Rax, R::Rdx)]),
    }
}

/// Saves `b` in the slot below the last, then zeroes slots 1..=n. `n` must be `f.slots`.
fn enter(f: &Function, n: u32, out: &mut Vec<X>) -> Result<(), String> {
    if n != f.slots {
        return Err(format!(
            "enter {n} does not match {} slots in {}",
            f.slots, f.name
        ));
    }
    let saved = u64::from(n) + 1;
    out.push(X::Push(R::Rbp));
    out.push(X::MovRR(R::Rbp, R::Rsp));
    out.push(X::AluI(AluOp::Sub, R::Rsp, disp(8 * saved)?));
    out.push(X::MovMR(word(saved)?, R::Rcx));
    if n > 0 {
        out.push(X::Alu(AluOp::Xor, R::Rdx, R::Rdx));
    }
    for k in 1..=n {
        out.push(X::MovMR(slot(f, k)?, R::Rdx));
    }
    Ok(())
}
