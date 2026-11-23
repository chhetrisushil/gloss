//! Lowering statements.
use super::Lowerer;
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
            } if self.is_number(value) || self.is_truth(value) => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(value)?;
                self.emit(Inst::StoreSlot(self.slots[name], Reg::A));
                Ok(())
            }
            StmtKind::Add {
                value,
                target: Target::Var(name),
            } => self.verb(s, name, value, VerbOp::Add),
            StmtKind::Subtract {
                value,
                target: Target::Var(name),
            } => self.verb(s, name, value, VerbOp::Sub),
            StmtKind::Multiply {
                by,
                target: Target::Var(name),
            } => self.verb(s, name, by, VerbOp::Mul),
            StmtKind::Divide {
                by,
                target: Target::Var(name),
            } => self.verb(s, name, by, VerbOp::Div),
            _ => Err(Diagnostic::error(
                self.src,
                s.line,
                "this compiler cannot build this statement yet",
            )),
        }
    }

    /// `name = name <op> operand`: the operand goes to `b`, the variable to `a`.
    fn verb(&mut self, s: &Stmt, name: &str, operand: &Expr, op: VerbOp) -> Result<(), Diagnostic> {
        self.line = s.line;
        self.emit(Inst::Line(s.line));
        self.expr(operand)?;
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::LoadSlot(Reg::A, self.slots[name]));
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
        self.emit(Inst::StoreSlot(self.slots[name], Reg::A));
        Ok(())
    }

    pub fn is_number(&self, e: &Expr) -> bool {
        matches!(
            self.types.expr_types.get(e.id.0 as usize),
            Some(Some(Ty::Number))
        )
    }

    fn show_item(&mut self, item: &Expr) -> Result<(), Diagnostic> {
        match &item.kind {
            ExprKind::Text(text) => {
                self.push_text(text.as_bytes());
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
            _ => Err(Diagnostic::error(
                self.src,
                item.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }
}
