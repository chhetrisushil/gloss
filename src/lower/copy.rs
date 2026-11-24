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
        self.copy_unless_fresh(e);
        Ok(())
    }

    /// The list a loop walks, in `a`. A list reached from a variable or loop variable (itself,
    /// or a field or item of it) is walked where it is, so changing the loop variable changes its
    /// row; any other list that already has an owner (a feature's result may be a caller's own
    /// value, as may a field of it) is copied first.
    pub fn walked_list(&mut self, e: &Expr) -> Result<(), Diagnostic> {
        self.expr(e)?;
        let mut root = e;
        while let ExprKind::Paren(of) | ExprKind::FieldOf { of, .. } | ExprKind::ItemOf { of, .. } =
            &root.kind
        {
            root = &**of;
        }
        if !matches!(root.kind, ExprKind::Name(_)) {
            self.copy_unless_fresh(e);
        }
        Ok(())
    }

    /// Replaces the list or record in `a`, the value of `e`, by a copy, unless `e` built it.
    fn copy_unless_fresh(&mut self, e: &Expr) {
        let ty = match self.types.expr_types.get(e.id.0 as usize) {
            Some(Some(ty @ (Ty::Record(_) | Ty::List(_)))) => ty.clone(),
            Some(Some(Ty::Table { record, .. })) => Ty::List(Box::new(Ty::Record(record.clone()))),
            _ => return,
        };
        if Self::is_fresh(e) {
            return;
        }
        let desc = self.type_desc(&ty);
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("copy_value");
    }
}
