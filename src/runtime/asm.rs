//! A small assembler for writing runtime routines as IR.

use crate::ir::{Cond, Data, Function, Global, Inst, Label, Reg};
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

    pub fn gload(&mut self, to: Reg, g: Global) {
        self.i(AddressOf(to, Data::Global(g)));
        self.i(Load {
            to,
            base: to,
            offset: 0,
        });
    }

    /// Stores `from` into a global, using `tmp` (another register) for its address.
    pub fn gstore(&mut self, g: Global, from: Reg, tmp: Reg) {
        self.i(AddressOf(tmp, Data::Global(g)));
        self.i(Store {
            base: tmp,
            offset: 0,
            from,
        });
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

    /// Pushes the address of the runtime text `n` (see `runtime::TEXTS`).
    pub fn push_text(&mut self, n: u32) {
        self.i(AddressOf(A, Data::Text(n)));
        self.i(Push(A));
    }

    /// Pushes the address of the bytes of the text argument `n`, then its length.
    pub fn push_text_arg(&mut self, n: u32) {
        self.i(LoadArg(A, n));
        self.i(AddImm(A, 8));
        self.i(Push(A));
        self.i(LoadArg(A, n));
        self.i(Load {
            to: A,
            base: A,
            offset: 0,
        });
        self.i(Push(A));
    }

    pub fn call(&mut self, name: &str) {
        self.i(Call(name.to_string()));
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
