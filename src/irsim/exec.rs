//! One instruction at a time.

use super::machine::Machine;
use crate::ir::Inst;

impl Machine<'_> {
    /// Runs one instruction; gives the result when the outermost call returns.
    pub fn step(&mut self) -> Option<i64> {
        let module = self.module;
        let frame = self.frame();
        let inst = &module.functions[frame.func].body[frame.pc];
        frame.pc += 1;
        match inst {
            Inst::Line(_) | Inst::Mark(_) => {}
            Inst::Const(r, value) => self.set(*r, *value),
            Inst::Return => return self.leave(),
            Inst::Enter(n) => assert_eq!(*n as usize, self.frame().slots.len()),
            Inst::LoadSlot(..)
            | Inst::StoreSlot(..)
            | Inst::Move { .. }
            | Inst::Push(_)
            | Inst::Pop(_)
            | Inst::LoadArg(..)
            | Inst::Call(_)
            | Inst::Int(_)
            | Inst::AddImm(..)
            | Inst::Shift(..)
            | Inst::Compare
            | Inst::JumpIf(..)
            | Inst::SetIf(_)
            | Inst::Jump(_)
            | Inst::AddressOf(..)
            | Inst::StackPointer(_)
            | Inst::Load { .. }
            | Inst::Store { .. }
            | Inst::LoadByte { .. }
            | Inst::StoreByte { .. }
            | Inst::StackCheck { .. } => todo!("a later commit"),
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
