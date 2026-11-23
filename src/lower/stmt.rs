//! Lowering statements.
use super::Lowerer;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, ExprKind};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::ir::{Inst, Reg};

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
            } if self.is_number(value) => {
                self.line = s.line;
                self.emit(Inst::Line(s.line));
                self.expr(value)?;
                self.emit(Inst::StoreSlot(self.slots[name], Reg::A));
                Ok(())
            }
            _ => Err(Diagnostic::error(
                self.src,
                s.line,
                "this compiler cannot build this statement yet",
            )),
        }
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
            _ => Err(Diagnostic::error(
                self.src,
                item.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }
}
