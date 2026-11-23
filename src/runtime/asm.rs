//! A small assembler for writing runtime routines as IR.

use crate::ir::{Cond, Function, Inst, Label, Reg};
use Inst::*;
use Reg::A;

/// A routine under construction: its instructions and its labels.
#[derive(Default)]
pub struct Asm {
    code: Vec<Inst>,
    labels: u32,
}

impl Asm {
    pub fn label(&mut self) -> Label {
        self.labels += 1;
        Label(self.labels)
    }

    pub fn i(&mut self, inst: Inst) {
        self.code.push(inst);
    }

    /// Compares `a` with `b` and jumps when `a cond b`.
    pub fn jump_if(&mut self, cond: Cond, to: Label) {
        self.i(Compare);
        self.i(JumpIf(cond, to));
    }

    pub fn push_const(&mut self, n: i64) {
        self.i(Const(A, n));
        self.i(Push(A));
    }

    pub fn push_arg(&mut self, n: u32) {
        self.i(LoadArg(A, n));
        self.i(Push(A));
    }

    pub fn push_slot(&mut self, n: u32) {
        self.i(LoadSlot(A, n));
        self.i(Push(A));
    }

    /// Ends the routine with a 0 result.
    pub fn finish(mut self, name: &str, params: u32, slots: u32) -> Function {
        self.i(Const(A, 0));
        self.i(Return);
        let mut body = vec![Enter(slots)];
        body.append(&mut self.code);
        Function {
            name: name.to_string(),
            params,
            slots,
            body,
        }
    }
}
