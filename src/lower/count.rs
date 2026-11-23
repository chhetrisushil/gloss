//! Lowering `for each i from s to e`.
//!
//! Both ends are worked out once, start first, into hidden slots. The counter is compared with the end before it is incremented, so a count ending at the top of
//! the range never adds past it.
use super::Lowerer;
use crate::ast::Expr;
use crate::ast::stmt::Stmt;
use crate::diag::Diagnostic;
use crate::ir::{Cond, Inst, IntOp, Reg};
use crate::number::SCALE;

impl Lowerer<'_> {
    pub fn count(
        &mut self,
        line: u32,
        var: &str,
        from: &Expr,
        to: &Expr,
        body: &[Stmt],
    ) -> Result<(), Diagnostic> {
        self.line = line;
        self.emit(Inst::Line(line));
        let start = self.once(from)?;
        let end = self.once(to)?;
        let counter = self.slots[var];
        let (top, next, exit) = (self.new_label(), self.new_label(), self.new_label());
        self.emit(Inst::LoadSlot(Reg::A, start));
        self.emit(Inst::StoreSlot(counter, Reg::A));
        self.emit(Inst::Mark(top));
        self.compare_counter(counter, end, Cond::Gt, exit);
        self.loops.push((next, exit));
        let done = self.stmts(body);
        self.loops.pop();
        done?;
        self.emit(Inst::Mark(next));
        self.compare_counter(counter, end, Cond::Eq, exit);
        self.emit(Inst::Const(Reg::B, SCALE));
        self.emit(Inst::Int(IntOp::Add));
        self.emit(Inst::StoreSlot(counter, Reg::A));
        self.emit(Inst::Jump(top));
        self.emit(Inst::Mark(exit));
        Ok(())
    }

    /// Works out `e` into a new hidden slot.
    fn once(&mut self, e: &Expr) -> Result<u32, Diagnostic> {
        let slot = self.new_temp();
        self.expr(e)?;
        self.emit(Inst::StoreSlot(slot, Reg::A));
        Ok(slot)
    }

    /// Loads the counter into `a` and the end into `b`, and jumps to `to` when `a cond b`.
    fn compare_counter(&mut self, counter: u32, end: u32, cond: Cond, to: crate::ir::Label) {
        self.emit(Inst::LoadSlot(Reg::A, counter));
        self.emit(Inst::LoadSlot(Reg::B, end));
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(cond, to));
    }
}
