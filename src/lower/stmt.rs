//! Lowering statements.
use super::list::item_target;
use super::record::Place;
use super::trace::Cell;
use super::{Lowerer, Mode};
use crate::ast::print;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, ExprKind};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::format;
use crate::ir::{Inst, Reg};

enum VerbOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl Lowerer<'_> {
    pub fn stmts(&mut self, body: &[Stmt]) -> Result<(), Diagnostic> {
        for s in body {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), Diagnostic> {
        match &s.kind {
            StmtKind::Note(_) => Ok(()),
            StmtKind::Show(items) => {
                self.mark_line(s.line);
                if self.mode == Mode::Trace {
                    return self.show_event(s.line, items);
                }
                for item in items {
                    self.show_item(item)?;
                }
                self.push_text(b"\n");
                self.call("out_text");
                Ok(())
            }
            StmtKind::Set {
                target: Target::Var(name),
                value,
            } if self.is_number(value)
                || self.is_truth(value)
                || self.is_text(value)
                || self.record_type(value).is_some()
                || self.list_item_type(value).is_some() =>
            {
                self.mark_line(s.line);
                self.store_value(value)?;
                if self.mode == Mode::Trace {
                    self.set_event(s.line, name, false);
                }
                self.emit(Inst::StoreSlot(self.slots[name], Reg::A));
                Ok(())
            }
            StmtKind::Set {
                target: target @ Target::Field { .. },
                value,
            } if self.place(target).is_some() => {
                self.mark_line(s.line);
                self.expr(value)?;
                let place = self.place(target).expect("just checked");
                if self.mode == Mode::Trace {
                    self.field_set_event(target, &place, &self.expr_ty(value));
                }
                self.store_place(&place);
                Ok(())
            }
            StmtKind::Append { value, list } => {
                self.mark_line(s.line);
                self.append(value, list)
            }
            StmtKind::Set { target, value } if item_target(target).is_some() => {
                let (position, _) = item_target(target).expect("just checked");
                self.mark_line(s.line);
                self.set_item(target, position, None, value)
            }
            StmtKind::Set { target, value } if self.item_field(target).is_some() => {
                let (position, _, offset) = self.item_field(target).expect("just checked");
                self.mark_line(s.line);
                self.set_item(target, position, Some(offset), value)
            }
            StmtKind::Ask { question, into } => {
                self.mark_line(s.line);
                self.expr(question)?;
                if self.mode == Mode::Trace {
                    return self.ask_event(s.line, into);
                }
                self.emit(Inst::Push(Reg::A));
                self.emit(Inst::Const(Reg::A, i64::from(s.line)));
                self.emit(Inst::Push(Reg::A));
                self.call("ask");
                self.emit(Inst::StoreSlot(self.slots[into], Reg::A));
                Ok(())
            }
            StmtKind::Add { value, target } => self.verb(s, target, value, VerbOp::Add),
            StmtKind::Subtract { value, target } => self.verb(s, target, value, VerbOp::Sub),
            StmtKind::Multiply { by, target } => self.verb(s, target, by, VerbOp::Mul),
            StmtKind::Divide { by, target } => self.verb(s, target, by, VerbOp::Div),
            StmtKind::If {
                branches,
                otherwise,
            } => self.if_chain(branches, otherwise.as_deref()),
            StmtKind::RepeatWhile { cond, body } => self.repeat_while(cond, body),
            StmtKind::ForEachRange {
                var,
                from,
                to,
                body,
            } => self.count(s.line, var, from, to, body),
            StmtKind::ForEachIn { var, list, body } => {
                self.mark_line(s.line);
                self.for_each(s.line, var, list, body)
            }
            StmtKind::Give(value) => {
                self.mark_line(s.line);
                self.expr(value)?;
                self.emit(Inst::Return);
                Ok(())
            }
            StmtKind::Run { feature, inputs } => {
                self.mark_line(s.line);
                self.feature_call(feature, inputs)
            }
            StmtKind::Skip | StmtKind::Stop => {
                let (skip, stop) = *self.loops.last().expect("skip and stop sit inside a loop");
                let target = if matches!(s.kind, StmtKind::Skip) {
                    skip
                } else {
                    stop
                };
                self.emit(Inst::Jump(target));
                Ok(())
            }
            StmtKind::Expect(cond) if self.mode == Mode::Test => {
                self.line = s.line;
                let fail = self.expect_failed(&print::expr(cond));
                self.condition(cond, fail)
            }
            StmtKind::Input(items) if self.mode == Mode::Test => {
                self.mark_line(s.line);
                for item in items {
                    self.expr(item)?;
                    self.emit(Inst::Push(Reg::A));
                    self.emit(Inst::Const(Reg::A, i64::from(s.line)));
                    self.emit(Inst::Push(Reg::A));
                    self.call("test_input");
                }
                Ok(())
            }
            StmtKind::Clear(name) => {
                self.mark_line(s.line);
                let index = self.table_index(name).expect("the checker found the table");
                self.clear_table(index);
                self.file_event(index, "clear");
                Ok(())
            }
            StmtKind::Load(name) => {
                self.mark_line(s.line);
                let index = self.table_index(name).expect("the checker found the table");
                self.read_table(index)
            }
            StmtKind::Save(name) => {
                self.mark_line(s.line);
                let index = self.table_index(name).expect("the checker found the table");
                self.save_table(index)
            }
            _ => Err(self.cannot_build(s)),
        }
    }

    /// `place = place <op> operand`: the operand goes to `b`, the current value to `a`.
    fn verb(
        &mut self,
        s: &Stmt,
        target: &Target,
        operand: &Expr,
        op: VerbOp,
    ) -> Result<(), Diagnostic> {
        let item = match (item_target(target), self.item_field(target)) {
            (Some((position, list)), _) => Some((position, list, None)),
            (None, Some((position, list, offset))) => Some((position, list, Some(offset))),
            (None, None) => None,
        };
        let place = self.place(target);
        if place.is_none() && item.is_none() {
            return Err(self.cannot_build(s));
        }
        self.mark_line(s.line);
        self.expr(operand)?;
        let at = match (&place, item) {
            (_, Some((position, list, field))) => Some(self.load_item(position, list, field)?),
            (Some(place), None) => {
                self.emit(Inst::Move {
                    to: Reg::B,
                    from: Reg::A,
                });
                self.emit(Inst::LoadSlot(Reg::A, place.slot));
                if let Some(offset) = place.field {
                    self.emit(Inst::Load {
                        to: Reg::A,
                        base: Reg::A,
                        offset,
                    });
                }
                None
            }
            (None, None) => unreachable!("checked above"),
        };
        // One line: a wrapped statement's continuation lines lose their indentation.
        let lines = format::print::stmt(s, 0);
        let text = lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
        let range = self.out_of_range(&text);
        let inst = match op {
            VerbOp::Add => Inst::NumAdd { range },
            VerbOp::Sub => Inst::NumSub { range },
            VerbOp::Mul => Inst::NumMul { range },
            VerbOp::Div => Inst::NumDiv {
                zero: self.divide_by_zero(&print::expr(operand)),
                range,
            },
        };
        self.emit(inst);
        if self.mode == Mode::Trace {
            match (target, at, &place) {
                (Target::Var(name), ..) => self.set_event(s.line, name, true),
                (_, Some((at, index)), _) => {
                    let cell = Cell::of(target, Some(index), at);
                    self.cell_set_event(&cell, &Ty::Number);
                }
                (_, None, Some(place)) => self.field_set_event(target, place, &Ty::Number),
                (_, None, None) => unreachable!("checked above"),
            }
        }
        match (at, &place) {
            (Some((at, _)), _) => self.store_item(at),
            (None, Some(place)) => self.store_place(place),
            (None, None) => unreachable!("checked above"),
        }
        Ok(())
    }

    /// The `set` event of the field a record variable's `place` names, whose new value of type
    /// `ty` is in `a` (`a` survives).
    fn field_set_event(&mut self, target: &Target, place: &Place, ty: &Ty) {
        let at = self.new_temp();
        self.emit(Inst::LoadSlot(Reg::B, place.slot));
        self.emit(Inst::AddImm(Reg::B, place.field.expect("a field place")));
        self.emit(Inst::StoreSlot(at, Reg::B));
        self.cell_set_event(&Cell::of(target, None, at), ty);
    }

    fn cannot_build(&self, s: &Stmt) -> Diagnostic {
        Diagnostic::error(
            self.src,
            s.line,
            "this compiler cannot build this statement yet",
        )
    }

    pub fn is_number(&self, e: &Expr) -> bool {
        matches!(
            self.types.expr_types.get(e.id.0 as usize),
            Some(Some(Ty::Number))
        )
    }

    pub fn is_text(&self, e: &Expr) -> bool {
        matches!(
            self.types.expr_types.get(e.id.0 as usize),
            Some(Some(Ty::Text))
        )
    }

    fn show_item(&mut self, item: &Expr) -> Result<(), Diagnostic> {
        if let ExprKind::Text(text) = &item.kind {
            self.push_text(text.as_bytes());
            self.call("out_text");
            return Ok(());
        }
        self.expr(item)?;
        self.show_loaded(item)
    }

    /// Shows `item`, whose value is in `a`, by its type.
    fn show_loaded(&mut self, item: &Expr) -> Result<(), Diagnostic> {
        match () {
            _ if self.is_text(item) => {
                self.emit(Inst::Push(Reg::A));
                self.call("out_text");
            }
            _ if self.is_number(item) => {
                self.emit(Inst::Push(Reg::A));
                self.call("out_number");
            }
            _ if self.is_truth(item) => self.show_truth(),
            _ if self.record_type(item).is_some() => {
                self.show_record(self.record_type(item).expect("just checked"));
            }
            _ if self.list_item_type(item).is_some() => {
                self.show_list(&self.list_item_type(item).expect("just checked"));
            }
            _ => {
                return Err(Diagnostic::error(
                    self.src,
                    item.line,
                    "this compiler cannot build this expression yet",
                ));
            }
        }
        Ok(())
    }

    /// A trace build's `show`: every item is evaluated into a hidden slot first, so no user
    /// code (a call that writes its own events) runs while the event is open; then the event
    /// is opened and each item written from its slot. A failure part-way through the items
    /// therefore comes before the event, which is then not written.
    fn show_event(&mut self, line: u32, items: &[Expr]) -> Result<(), Diagnostic> {
        let mut slots = Vec::new();
        for item in items {
            let slot = if matches!(item.kind, ExprKind::Text(_)) {
                None
            } else {
                self.expr(item)?;
                let slot = self.new_temp();
                self.emit(Inst::StoreSlot(slot, Reg::A));
                Some(slot)
            };
            slots.push(slot);
        }
        self.ev_open(line, "show");
        self.ev_key("text");
        self.ev_raw("\"");
        self.set_open(true);
        self.set_escaping(true);
        for (item, slot) in items.iter().zip(slots) {
            match slot {
                Some(slot) => {
                    self.emit(Inst::LoadSlot(Reg::A, slot));
                    self.show_loaded(item)?;
                }
                None => self.show_item(item)?,
            }
        }
        self.set_escaping(false);
        self.set_open(false);
        self.ev_raw("\"");
        self.ev_close();
        Ok(())
    }

    /// A traced `ask`, with the question text in `a`: it goes into a hidden slot first (no user
    /// code runs while an event is open), `ask` shows it as a `show` event and reads the
    /// answer, which is stored in the variable, then the `ask` event records both.
    fn ask_event(&mut self, line: u32, into: &str) -> Result<(), Diagnostic> {
        let question = self.new_temp();
        self.emit(Inst::StoreSlot(question, Reg::A));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(line)));
        self.emit(Inst::Push(Reg::A));
        self.call("ask");
        let answer = self.slots[into];
        self.emit(Inst::StoreSlot(answer, Reg::A));
        self.set_made(into);
        self.ev_open(line, "ask");
        self.ev_key("name");
        self.ev_fixed_text(into);
        self.ev_key("question");
        self.ev_text_from_slot(question);
        self.ev_key("answer");
        self.ev_text_from_slot(answer);
        self.ev_close();
        Ok(())
    }
}
