//! Lowering lists. A list is the address of a header `[count, capacity, items]`; `items` is
//! the address of `capacity` 8-byte slots (see `ir::Desc`).
use super::Lowerer;
use crate::ast::{Expr, ExprKind};
use crate::diag::Diagnostic;
use crate::ir::{Inst, IntOp, Reg};
use crate::number::SCALE;

/// The offset of the count, the capacity and the items address in a header.
const COUNT: i32 = 0;
const ITEMS: i32 = 16;

/// A new list has room for at least this many items.
const MIN_ROOM: usize = 4;

impl Lowerer<'_> {
    /// Whether `e` builds a list that `set` can store without a copy: `empty list of T`, or
    /// `list of …` whose record items are built in place (copying arrives in Task 22).
    pub fn builds_list(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::EmptyList(_) => true,
            ExprKind::BuildList(items) => items.iter().all(|i| self.stores_unshared(i)),
            ExprKind::Paren(inner) => self.builds_list(inner),
            _ => false,
        }
    }

    /// Whether `e` can be stored in a list as it is: it is not a record, or it is a new one.
    pub fn stores_unshared(&self, e: &Expr) -> bool {
        self.record_type(e).is_none() || Self::builds_record(e)
    }

    /// `list of a, b, …` and `empty list of T`: the header, then each item in the order written.
    pub fn build_list(&mut self, items: &[Expr]) -> Result<(), Diagnostic> {
        let head = self.new_temp();
        self.emit(Inst::Const(Reg::A, items.len().max(MIN_ROOM) as i64));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("list_new");
        self.emit(Inst::StoreSlot(head, Reg::A));
        for (i, item) in items.iter().enumerate() {
            self.expr(item)?;
            self.emit(Inst::Move {
                to: Reg::B,
                from: Reg::A,
            });
            self.emit(Inst::LoadSlot(Reg::A, head));
            self.emit(Inst::Load {
                to: Reg::A,
                base: Reg::A,
                offset: ITEMS,
            });
            self.emit(Inst::Store {
                base: Reg::A,
                offset: 8 * i as i32,
                from: Reg::B,
            });
        }
        self.emit(Inst::Const(Reg::B, items.len() as i64));
        self.emit(Inst::LoadSlot(Reg::A, head));
        self.emit(Inst::Store {
            base: Reg::A,
            offset: COUNT,
            from: Reg::B,
        });
        Ok(())
    }

    /// `count of xs`, as a number.
    pub fn list_count(&mut self, of: &Expr) -> Result<(), Diagnostic> {
        self.expr(of)?;
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: COUNT,
        });
        self.emit(Inst::Const(Reg::B, SCALE));
        self.emit(Inst::Int(IntOp::Mul));
        Ok(())
    }
}
