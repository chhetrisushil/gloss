//! Lowering lists. A list is the address of a header `[count, capacity, items]`; `items` is
//! the address of `capacity` 8-byte slots (see `ir::Desc`).
use super::Lowerer;
use crate::ast::print;
use crate::ast::{Expr, ExprKind, Fact};
use crate::diag::Diagnostic;
use crate::ir::{Cond, Inst, IntOp, Reg, Shift};
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

    /// `first of xs` or `last of xs`; an empty list is failure 2.
    pub fn list_end(&mut self, fact: Fact, of: &Expr) -> Result<(), Diagnostic> {
        let list = self.new_temp();
        self.expr(of)?;
        self.emit(Inst::StoreSlot(list, Reg::A));
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: COUNT,
        });
        self.emit(Inst::Const(Reg::B, 0));
        self.emit(Inst::Compare);
        let which = if fact == Fact::First { "first" } else { "last" };
        let empty = self.no_end_item(which, &print::expr(of));
        self.emit(Inst::JumpIf(Cond::Eq, empty));
        if fact == Fact::First {
            self.emit(Inst::Const(Reg::A, 0));
        } else {
            self.emit(Inst::AddImm(Reg::A, -1));
        }
        self.slot_address(list);
        self.load_a();
        Ok(())
    }

    /// `item p of xs`.
    pub fn item_of(&mut self, position: &Expr, of: &Expr) -> Result<(), Diagnostic> {
        let list = self.new_temp();
        self.expr(of)?;
        self.emit(Inst::StoreSlot(list, Reg::A));
        self.item_address(list, &print::expr(of), position)?;
        self.load_a();
        Ok(())
    }

    /// The address of item `position` of the list in slot `list`, whose source is `source`,
    /// in `a`. The position must be whole (failure 3) and in 1 to the count (failure 2).
    pub fn item_address(
        &mut self,
        list: u32,
        source: &str,
        position: &Expr,
    ) -> Result<(), Diagnostic> {
        let (pos, index) = (self.new_temp(), self.new_temp());
        self.expr(position)?;
        self.emit(Inst::StoreSlot(pos, Reg::A));
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::NumWhole);
        self.emit(Inst::Compare);
        let fractional = self.bad_position(&print::expr(position), pos);
        self.emit(Inst::JumpIf(Cond::Ne, fractional));
        let missing = self.no_item(source, pos, list);
        self.emit(Inst::LoadSlot(Reg::A, pos));
        self.emit(Inst::Const(Reg::B, SCALE));
        self.emit(Inst::Int(IntOp::Div));
        self.emit(Inst::StoreSlot(index, Reg::A));
        self.emit(Inst::Const(Reg::B, 1));
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(Cond::Lt, missing));
        self.emit(Inst::LoadSlot(Reg::B, list));
        self.emit(Inst::Load {
            to: Reg::B,
            base: Reg::B,
            offset: COUNT,
        });
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(Cond::Gt, missing));
        self.emit(Inst::LoadSlot(Reg::A, index));
        self.emit(Inst::AddImm(Reg::A, -1));
        self.slot_address(list);
        Ok(())
    }

    /// With the zero-based index in `a`: the address of that slot of the list in `list`.
    fn slot_address(&mut self, list: u32) {
        self.emit(Inst::Shift(Shift::Left, 3));
        self.emit(Inst::LoadSlot(Reg::B, list));
        self.emit(Inst::Load {
            to: Reg::B,
            base: Reg::B,
            offset: ITEMS,
        });
        self.emit(Inst::Int(IntOp::Add));
    }

    /// Replaces the address in `a` by the slot's value.
    fn load_a(&mut self) {
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: 0,
        });
    }
}
