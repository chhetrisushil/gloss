//! Copy semantics: a list or record is copied when it is stored, so two variables never share
//! one. A value that was just built (`list of`, `empty list of`, `E with`) has no other owner
//! and is stored as it is.
use super::Lowerer;
use crate::ast::{Expr, ExprKind};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::ir::{Data, Inst, Reg};

impl Lowerer<'_> {
    /// Whether `e` builds its list or record itself.
    fn is_fresh(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::BuildRecord { .. } | ExprKind::EmptyList(_) | ExprKind::BuildList(_) => true,
            ExprKind::Paren(inner) => Self::is_fresh(inner),
            _ => false,
        }
    }

    /// The value of `e` in `a`, ready to be stored: a list or record that already has an owner
    /// is copied, with the records inside a list.
    pub fn store_value(&mut self, e: &Expr) -> Result<(), Diagnostic> {
        self.expr(e)?;
        let ty = match self.types.expr_types.get(e.id.0 as usize) {
            Some(Some(ty @ (Ty::Record(_) | Ty::List(_)))) => ty.clone(),
            _ => return Ok(()),
        };
        if Self::is_fresh(e) {
            return Ok(());
        }
        let desc = self.type_desc(&ty);
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("copy_value");
        Ok(())
    }
}
