//! Lowering statements.
use super::Lowerer;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::{Expr, ExprKind};
use crate::diag::Diagnostic;
use crate::ir::Inst;

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
                "this statement is not lowered yet",
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
            _ => Err(Diagnostic::error(
                self.src,
                item.line,
                "this expression is not lowered yet",
            )),
        }
    }
}
