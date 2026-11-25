//! Lowering `for each i from s to e`.
//!
//! Both ends are worked out once, start first, into hidden slots; each must be whole
//! (failure 3, so a fractional start is reported before the end is even worked out). The
//! counter is compared with the end before it is incremented, so a count ending at the top of
//! the range never adds past it.
use super::Lowerer;
use crate::ast::Expr;
use crate::ast::print;
use crate::ast::stmt::Stmt;
use crate::check::Ty;
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
        self.mark_line(line);
        let start = self.whole_end("start", from)?;
        let end = self.whole_end("end", to)?;
        let counter = self.slots[var];
        let turn = self.begin_turns();
        if let Some(turn) = turn {
            self.emit(Inst::Const(Reg::A, 0));
            self.emit(Inst::StoreSlot(turn, Reg::A));
        }
        let (top, next, exit) = (self.new_label(), self.new_label(), self.new_label());
        self.emit(Inst::LoadSlot(Reg::A, start));
        self.emit(Inst::StoreSlot(counter, Reg::A));
        self.emit(Inst::Mark(top));
        self.compare_counter(counter, end, Cond::Gt, exit);
        if let Some(turn) = turn {
            self.emit(Inst::LoadSlot(Reg::A, turn));
            self.emit(Inst::AddImm(Reg::A, 1));
            self.emit(Inst::StoreSlot(turn, Reg::A));
            self.loop_event(line, var, &Ty::Number, turn);
        }
        self.loop_body(var, Ty::Number, (next, exit), body)?;
        self.emit(Inst::Mark(next));
        self.compare_counter(counter, end, Cond::Eq, exit);
        self.emit(Inst::Const(Reg::B, SCALE));
        self.emit(Inst::Int(IntOp::Add));
        self.emit(Inst::StoreSlot(counter, Reg::A));
        self.emit(Inst::Jump(top));
        self.emit(Inst::Mark(exit));
        Ok(())
    }

    /// Works out `e` into a new hidden slot and fails (3) when it is not whole.
    fn whole_end(&mut self, what: &str, e: &Expr) -> Result<u32, Diagnostic> {
        let slot = self.new_temp();
        self.expr(e)?;
        self.emit(Inst::StoreSlot(slot, Reg::A));
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::NumWhole);
        self.emit(Inst::Compare);
        let fail = self.not_whole(what, &print::expr(e), slot);
        self.emit(Inst::JumpIf(Cond::Ne, fail));
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

impl Lowerer<'_> {
    /// Lowers `body` with `var` typed `ty` and `labels` as the loop to leave or repeat; the
    /// type the name had before is put back afterwards.
    pub fn loop_body(
        &mut self,
        var: &str,
        ty: Ty,
        labels: (crate::ir::Label, crate::ir::Label),
        body: &[Stmt],
    ) -> Result<(), Diagnostic> {
        let before = self.vars.insert(var.to_string(), ty);
        self.loops.push(labels);
        let done = self.stmts(body);
        self.loops.pop();
        match before {
            Some(ty) => self.vars.insert(var.to_string(), ty),
            None => self.vars.remove(var),
        };
        done
    }
}
