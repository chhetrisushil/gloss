//! Lowering expressions: the value ends up in register `a`.
use super::Lowerer;
use crate::ast::{Expr, ExprKind};
use crate::diag::Diagnostic;
use crate::ir::{Inst, Reg};

impl Lowerer<'_> {
    pub fn expr(&mut self, e: &Expr) -> Result<(), Diagnostic> {
        match &e.kind {
            ExprKind::Number(n) => {
                self.emit(Inst::Const(Reg::A, n.0));
                Ok(())
            }
            // A negated literal is a constant; the literal is in range, so its negation is.
            ExprKind::Negate(inner) if matches!(inner.kind, ExprKind::Number(_)) => {
                let ExprKind::Number(n) = &inner.kind else {
                    unreachable!()
                };
                self.emit(Inst::Const(Reg::A, -n.0));
                Ok(())
            }
            _ => Err(Diagnostic::error(
                self.src,
                e.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }
}
