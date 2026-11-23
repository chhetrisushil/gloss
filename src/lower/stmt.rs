//! Lowering statements.
use super::Lowerer;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::{Expr, ExprKind};
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
                self.emit(Inst::Line(s.line));
                for item in items {
                    self.show_item(item)?;
                }
                self.push_text(b"\n");
                self.call("out_text");
                Ok(())
            }
            _ => Err(Diagnostic::error(
                self.src,
                s.line,
                "this compiler cannot build this statement yet",
            )),
        }
    }

    fn show_item(&mut self, item: &Expr) -> Result<(), Diagnostic> {
        match &item.kind {
            ExprKind::Text(text) => {
                self.push_text(text.as_bytes());
                self.call("out_text");
                Ok(())
            }
            ExprKind::Number(_) | ExprKind::Negate(_) => {
                self.expr(item)?;
                self.emit(Inst::Push(Reg::A));
                self.emit(Inst::Const(Reg::A, i64::from(item.line)));
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
