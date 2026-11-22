//! One instruction at a time.

use super::machine::Machine;
use crate::ir::{Inst, IntOp, Shift};

fn int_op(op: IntOp, a: i64, b: i64) -> i64 {
    if matches!(op, IntOp::Div | IntOp::Rem) {
        // Ruling M2-R7: a CPU divide faults on zero and on MIN / -1, and the backends
        // do not special-case negative divisors.
        assert!(
            b > 0,
            "irsim: M2-R7: Div and Rem need a divisor above 0, got {b}"
        );
    }
    match op {
        IntOp::Add => a.wrapping_add(b),
        IntOp::Sub => a.wrapping_sub(b),
        IntOp::Mul => a.wrapping_mul(b),
        IntOp::Div => a.wrapping_div(b),
        IntOp::Rem => a.wrapping_rem(b),
        IntOp::And => a & b,
        IntOp::Or => a | b,
    }
}

fn shift(kind: Shift, value: i64, n: u8) -> i64 {
    assert!(n < 64, "irsim: shift count {n} is not below 64");
    match kind {
        Shift::Left => value.wrapping_shl(n as u32),
        Shift::Right => value >> n,
        Shift::RightLogical => ((value as u64) >> n) as i64,
    }
}

impl Machine<'_> {
    /// Runs one instruction; gives the result when the outermost call returns.
    pub fn step(&mut self) -> Option<i64> {
        let module = self.module;
        let frame = self.frame();
        let inst = &module.functions[frame.func].body[frame.pc];
        frame.pc += 1;
        if !matches!(inst, Inst::Line(_)) {
            self.compared = self.fresh.take();
        }
        match inst {
            Inst::Line(_) | Inst::Mark(_) => {}
            Inst::Const(r, value) => self.set(*r, *value),
            Inst::LoadSlot(r, n) => {
                let value = self.frame().slots[*n as usize - 1];
                self.set(*r, value);
            }
            Inst::StoreSlot(n, r) => {
                let value = self.reg(*r);
                self.frame().slots[*n as usize - 1] = value;
            }
            Inst::Move { to, from } => self.set(*to, self.reg(*from)),
            Inst::Push(r) => self.push(self.reg(*r)),
            Inst::Pop(r) => {
                let value = self.pop();
                self.set(*r, value);
            }
            Inst::Int(op) => self.a = int_op(*op, self.a, self.b),
            Inst::AddImm(r, n) => self.set(*r, self.reg(*r).wrapping_add(*n as i64)),
            Inst::Shift(kind, n) => self.a = shift(*kind, self.a, *n),
            Inst::Compare => self.fresh = Some((self.a, self.b)),
            Inst::SetIf(cond) => self.a = self.holds(*cond) as i64,
            Inst::LoadArg(r, n) => {
                let value = self.frame().args[*n as usize - 1];
                self.set(*r, value);
            }
            Inst::JumpIf(cond, label) => {
                if self.holds(*cond) {
                    self.jump(*label);
                }
            }
            Inst::Jump(label) => self.jump(*label),
            Inst::Call(name) => self.enter(name),
            Inst::AddressOf(r, data) => {
                let addr = self.memory.address_of(*data);
                self.set(*r, addr as i64);
            }
            Inst::StackPointer(r) => self.set(*r, self.sp as i64),
            Inst::Load { to, base, offset } => {
                let addr = self.effective(*base, *offset);
                self.check(addr, 8, false);
                let value = self.memory.read(addr);
                self.set(*to, value);
            }
            Inst::Store { base, offset, from } => {
                let addr = self.effective(*base, *offset);
                self.check(addr, 8, true);
                self.memory.write(addr, self.reg(*from));
            }
            // A loaded byte is zero-extended.
            Inst::LoadByte { to, base, offset } => {
                let addr = self.effective(*base, *offset);
                self.check(addr, 1, false);
                let value = self.memory.read_byte(addr);
                self.set(*to, i64::from(value));
            }
            Inst::StoreByte { base, offset, from } => {
                let addr = self.effective(*base, *offset);
                self.check(addr, 1, true);
                self.memory.write_byte(addr, self.reg(*from) as u8);
            }
            Inst::StackCheck { bytes, fail } => {
                if self.stack_overflows(*bytes) {
                    self.jump(*fail);
                }
            }
            Inst::Return => return self.leave(),
            Inst::Enter(n) => assert_eq!(*n as usize, self.frame().slots.len()),
            Inst::NumAdd { .. }
            | Inst::NumSub { .. }
            | Inst::NumNeg { .. }
            | Inst::NumMul { .. }
            | Inst::NumDiv { .. }
            | Inst::NumRem { .. }
            | Inst::NumWhole
            | Inst::System(_) => todo!("Task 3b"),
        }
        None
    }
}
