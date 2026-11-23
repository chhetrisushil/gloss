//! Lowering statements.
use super::Lowerer;
use super::list::item_target;
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
                self.line = s.line;
                self.emit(Inst::Line(s.line));
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
                || Self::builds_record(value)
                || self.builds_list(value) =>
            {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(value)?;
                self.emit(Inst::StoreSlot(self.slots[name], Reg::A));
                Ok(())
            }
            StmtKind::Set {
                target: target @ Target::Field { .. },
                value,
            } if self.place(target).is_some() => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(value)?;
                let place = self.place(target).expect("just checked");
                self.store_place(&place);
                Ok(())
            }
            StmtKind::Append { value, list } if self.stores_unshared(value) => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.append(value, list)
            }
            StmtKind::Set {
                target: Target::Item { position, of },
                value,
            } if self.stores_unshared(value) && matches!(**of, Target::Var(_)) => {
                let Target::Var(list) = &**of else {
                    unreachable!()
                };
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.set_item(position, list, value)
            }
            StmtKind::Ask { question, into } => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(question)?;
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
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.for_each(var, list, body)
            }
            StmtKind::Give(value) => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(value)?;
                self.emit(Inst::Return);
                Ok(())
            }
            StmtKind::Run { feature, inputs } => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
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
        let item = item_target(target);
        let place = self.place(target);
        if place.is_none() && item.is_none() {
            return Err(self.cannot_build(s));
        }
        self.line = s.line;
        self.emit(Inst::Line(s.line));
        self.expr(operand)?;
        let at = match (&place, item) {
            (_, Some((position, list))) => Some(self.load_item(position, list)?),
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
        match (at, &place) {
            (Some(at), _) => self.store_item(at),
            (None, Some(place)) => self.store_place(place),
            (None, None) => unreachable!("checked above"),
        }
        Ok(())
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
        match &item.kind {
            ExprKind::Text(text) => {
                self.push_text(text.as_bytes());
                self.call("out_text");
                Ok(())
            }
            _ if self.is_text(item) => {
                self.expr(item)?;
                self.emit(Inst::Push(Reg::A));
                self.call("out_text");
                Ok(())
            }
            _ if self.is_number(item) => {
                self.expr(item)?;
                self.emit(Inst::Push(Reg::A));
                self.call("out_number");
                Ok(())
            }
            _ if self.is_truth(item) => {
                self.expr(item)?;
                self.show_truth();
                Ok(())
            }
            _ if self.record_type(item).is_some() => {
                self.expr(item)?;
                self.show_record(self.record_type(item).expect("just checked"));
                Ok(())
            }
            _ if self.list_item_type(item).is_some() => {
                self.expr(item)?;
                self.show_list(&self.list_item_type(item).expect("just checked"));
                Ok(())
            }
            _ => Err(Diagnostic::error(
                self.src,
                item.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }
}
