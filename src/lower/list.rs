//! Lowering lists. A list is the address of a header `[count, capacity, items]`; `items` is
//! the address of `capacity` 8-byte slots (see `ir::Desc`).
use super::trace::Cell;
use super::{Lowerer, Mode};
use crate::ast::print;
use crate::ast::stmt::{Stmt, Target};
use crate::ast::{Expr, Fact};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::ir::{Cond, Data, Desc, Inst, IntOp, Reg, Shift};
use crate::number::SCALE;

/// The offset of the count, the capacity and the items address in a header.
const COUNT: i32 = 0;
const ITEMS: i32 = 16;

/// The position and list name of an item target on a list variable, `item p of xs`.
pub fn item_target(target: &Target) -> Option<(&Expr, &str)> {
    match target {
        Target::Item { position, of } => match &**of {
            Target::Var(name) => Some((position, name)),
            _ => None,
        },
        _ => None,
    }
}

/// A new list has room for at least this many items.
const MIN_ROOM: usize = 4;

impl Lowerer<'_> {
    /// The position, list name and field offset of `field of item p of xs`, a target.
    pub fn item_field<'t>(&self, target: &'t Target) -> Option<(&'t Expr, &'t str, i32)> {
        let Target::Field { field, of } = target else {
            return None;
        };
        let (position, list) = item_target(of)?;
        let record = match (self.vars.get(list), self.table_index(list)) {
            (Some(Ty::List(item)), _) => match &**item {
                Ty::Record(record) => record.clone(),
                _ => return None,
            },
            (None, Some(i)) => self.program.tables().nth(i)?.record.clone(),
            _ => return None,
        };
        Some((position, list, self.field_offset(&record, field)))
    }

    /// The type of `e`, which the checker found.
    pub fn expr_ty(&self, e: &Expr) -> Ty {
        match self.types.expr_types.get(e.id.0 as usize) {
            Some(Some(ty)) => ty.clone(),
            _ => unreachable!("the checker types every value it lets through"),
        }
    }

    /// The item type of `e`, when it is a list.
    pub fn list_item_type(&self, e: &Expr) -> Option<Ty> {
        match self.types.expr_types.get(e.id.0 as usize) {
            Some(Some(Ty::List(item))) => Some((**item).clone()),
            Some(Some(Ty::Table { record, .. })) => Some(Ty::Record(record.clone())),
            _ => None,
        }
    }

    /// The descriptor of values of type `ty`: scalars, records and lists of them.
    pub fn type_desc(&mut self, ty: &Ty) -> u32 {
        match ty {
            Ty::Record(name) => self.record_desc(name),
            Ty::List(item) => {
                let item = self.type_desc(item);
                self.module.desc(Desc::List(item))
            }
            Ty::Table { record, .. } => {
                self.type_desc(&Ty::List(Box::new(Ty::Record(record.clone()))))
            }
            Ty::Text => self.module.desc(Desc::Text),
            Ty::Truth => self.module.desc(Desc::Truth),
            _ => self.module.desc(Desc::Number),
        }
    }

    /// Shows the list in `a`, whose items have type `item`.
    pub fn show_list(&mut self, item: &Ty) {
        let desc = self.type_desc(&Ty::List(Box::new(item.clone())));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        self.call("show_value");
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
            self.store_value(item)?;
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
    /// in `a`; the slot returned holds the whole position. The position must be whole
    /// (failure 3) and in 1 to the count (failure 2).
    pub fn item_address(
        &mut self,
        list: u32,
        source: &str,
        position: &Expr,
    ) -> Result<u32, Diagnostic> {
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
        Ok(index)
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

    /// `append v to xs`: the runtime puts it at the end and grows the list when it is full.
    pub fn append(&mut self, value: &Expr, list: &str) -> Result<(), Diagnostic> {
        let item = self.new_temp();
        self.store_value(value)?;
        self.emit(Inst::StoreSlot(item, Reg::A));
        let slot = self.list_slot(list);
        self.emit(Inst::LoadSlot(Reg::A, slot));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::LoadSlot(Reg::A, item));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("list_push");
        if self.mode == Mode::Trace {
            self.append_event(list, (slot, item), &self.expr_ty(value));
        }
        Ok(())
    }

    /// With a new value in `a`: finds item `position` of list `list`, remembers its address
    /// in the first slot returned (the second holds the whole position), and leaves its value
    /// in `a` and the new value in `b`. With a `field`, the address is that field's of the
    /// item's record.
    pub fn load_item(
        &mut self,
        position: &Expr,
        list: &str,
        field: Option<i32>,
    ) -> Result<(u32, u32), Diagnostic> {
        let (value, at) = (self.new_temp(), self.new_temp());
        self.emit(Inst::StoreSlot(value, Reg::A));
        let slot = self.list_slot(list);
        let index = self.item_address(slot, list, position)?;
        if let Some(offset) = field {
            self.load_a();
            self.emit(Inst::AddImm(Reg::A, offset));
        }
        self.emit(Inst::StoreSlot(at, Reg::A));
        self.load_a();
        self.emit(Inst::LoadSlot(Reg::B, value));
        Ok((at, index))
    }

    /// Stores the value in `a` into the item whose address is in slot `at`.
    pub fn store_item(&mut self, at: u32) {
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::LoadSlot(Reg::A, at));
        self.emit(Inst::Store {
            base: Reg::A,
            offset: 0,
            from: Reg::B,
        });
    }

    /// `set item p of xs to v` (or a field of the item, at byte offset `field`): the value is
    /// worked out first, then the position is checked. A trace build writes the `set` event
    /// after the checks, before the store.
    pub fn set_item(
        &mut self,
        target: &Target,
        position: &Expr,
        field: Option<i32>,
        value: &Expr,
    ) -> Result<(), Diagnostic> {
        let (held, list) = (self.new_temp(), target.root());
        self.store_value(value)?;
        self.emit(Inst::StoreSlot(held, Reg::A));
        let slot = self.list_slot(list);
        let index = self.item_address(slot, list, position)?;
        if self.mode == Mode::Trace {
            let (raw, at) = (self.new_temp(), self.new_temp());
            self.emit(Inst::StoreSlot(raw, Reg::A));
            if let Some(offset) = field {
                self.load_a();
                self.emit(Inst::AddImm(Reg::A, offset));
            }
            self.emit(Inst::StoreSlot(at, Reg::A));
            self.emit(Inst::LoadSlot(Reg::A, held));
            let ty = self.expr_ty(value);
            self.cell_set_event(&Cell::of(target, Some(index), at), &ty);
            self.emit(Inst::LoadSlot(Reg::A, raw));
        }
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::LoadSlot(Reg::A, held));
        if let Some(offset) = field {
            self.emit(Inst::Load {
                to: Reg::B,
                base: Reg::B,
                offset: 0,
            });
            self.emit(Inst::Store {
                base: Reg::B,
                offset,
                from: Reg::A,
            });
            return Ok(());
        }
        self.emit(Inst::Store {
            base: Reg::B,
            offset: 0,
            from: Reg::A,
        });
        Ok(())
    }

    /// `for each x in xs`: the list and the position live in hidden slots; each turn puts the
    /// item at the position in `x` (a record's address, so changing its fields changes the
    /// list's record). The checker refuses to append to, set, clear or load the list, or replace
    /// one of its items, while the loop walks it (M2-R34), so the count is read afresh without
    /// risk and the variable keeps the item it was given.
    pub fn for_each(
        &mut self,
        line: u32,
        var: &str,
        list: &Expr,
        body: &[Stmt],
    ) -> Result<(), Diagnostic> {
        let (held, at) = (self.new_temp(), self.new_temp());
        self.walked_list(list)?;
        self.emit(Inst::StoreSlot(held, Reg::A));
        self.emit(Inst::Const(Reg::A, 0));
        self.emit(Inst::StoreSlot(at, Reg::A));
        let (top, next, exit) = (self.new_label(), self.new_label(), self.new_label());
        self.emit(Inst::Mark(top));
        self.emit(Inst::LoadSlot(Reg::A, at));
        self.emit(Inst::LoadSlot(Reg::B, held));
        self.emit(Inst::Load {
            to: Reg::B,
            base: Reg::B,
            offset: COUNT,
        });
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(Cond::Ge, exit));
        self.slot_address(held);
        self.load_a();
        self.emit(Inst::StoreSlot(self.slots[var], Reg::A));
        let item = self
            .list_item_type(list)
            .expect("the checker knows the list");
        if let Some(turn) = self.begin_turns() {
            self.emit(Inst::LoadSlot(Reg::A, at));
            self.emit(Inst::AddImm(Reg::A, 1));
            self.emit(Inst::StoreSlot(turn, Reg::A));
            self.loop_event(line, var, &item, turn);
        }
        self.loop_body(var, item, (next, exit), body)?;
        self.emit(Inst::Line(line));
        self.emit(Inst::Mark(next));
        self.emit(Inst::LoadSlot(Reg::A, at));
        self.emit(Inst::AddImm(Reg::A, 1));
        self.emit(Inst::StoreSlot(at, Reg::A));
        self.emit(Inst::Jump(top));
        self.emit(Inst::Mark(exit));
        Ok(())
    }
}
