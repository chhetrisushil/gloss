//! Lowering truths and conditions: a truth is 1 or 0 in register `a`.
use super::Lowerer;
use crate::ast::stmt::Stmt;
use crate::ast::{CompareOp, Expr, ExprKind};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::ir::{Cond, Inst, IntOp, Label, Reg};

impl Lowerer<'_> {
    pub fn is_truth(&self, e: &Expr) -> bool {
        matches!(
            self.types.expr_types.get(e.id.0 as usize),
            Some(Some(Ty::Truth))
        )
    }

    /// Lowers a truth-valued expression that is not a name or a literal, if it is one this
    /// compiler can build; `None` for the rest.
    pub fn truth(&mut self, e: &Expr) -> Option<Result<(), Diagnostic>> {
        Some(match &e.kind {
            ExprKind::Truth(v) => {
                self.emit(Inst::Const(Reg::A, i64::from(*v)));
                Ok(())
            }
            ExprKind::Compare { op, left, right }
                if self.is_number(left) || self.is_truth(left) =>
            {
                self.compare(*op, left, right)
            }
            // The checker lets only `is` and `is not` compare text.
            ExprKind::Compare { op, left, right } if self.is_text(left) => {
                self.compare_text(*op == CompareOp::IsNot, left, right)
            }
            ExprKind::IsNumber { value, negated } => self.is_number_check(*negated, value),
            ExprKind::Not(inner) => self.not(inner),
            ExprKind::And(parts) => self.join(parts, 0),
            ExprKind::Or(parts) => self.join(parts, 1),
            _ => return None,
        })
    }

    fn compare(&mut self, op: CompareOp, left: &Expr, right: &Expr) -> Result<(), Diagnostic> {
        self.operands(left, right)?;
        self.emit(Inst::Compare);
        self.emit(Inst::SetIf(match op {
            CompareOp::Is => Cond::Eq,
            CompareOp::IsNot => Cond::Ne,
            CompareOp::Greater => Cond::Gt,
            CompareOp::Less => Cond::Lt,
            CompareOp::AtLeast => Cond::Ge,
            CompareOp::AtMost => Cond::Le,
        }));
        Ok(())
    }

    /// Both texts are pushed for `text_equal`; `is not` flips its answer.
    fn compare_text(&mut self, negate: bool, left: &Expr, right: &Expr) -> Result<(), Diagnostic> {
        self.expr(left)?;
        self.emit(Inst::Push(Reg::A));
        self.expr(right)?;
        self.emit(Inst::Push(Reg::A));
        self.call("text_equal");
        if negate {
            self.emit(Inst::Move {
                to: Reg::B,
                from: Reg::A,
            });
            self.emit(Inst::Const(Reg::A, 1));
            self.emit(Inst::Int(IntOp::Sub));
        }
        Ok(())
    }

    /// `parse_number` gives 0 for a number: `is number` is 1 minus that, `is not number` is it.
    fn is_number_check(&mut self, negated: bool, value: &Expr) -> Result<(), Diagnostic> {
        self.expr(value)?;
        self.emit(Inst::Push(Reg::A));
        self.call("parse_number");
        if !negated {
            self.emit(Inst::Move {
                to: Reg::B,
                from: Reg::A,
            });
            self.emit(Inst::Const(Reg::A, 1));
            self.emit(Inst::Int(IntOp::Sub));
        }
        Ok(())
    }

    /// `a = 1 - a`.
    fn not(&mut self, inner: &Expr) -> Result<(), Diagnostic> {
        self.expr(inner)?;
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::Const(Reg::A, 1));
        self.emit(Inst::Int(IntOp::Sub));
        Ok(())
    }

    /// Left to right; stops at the first part that is `stop` (0 for `and`, 1 for `or`), which
    /// is then the value.
    fn join(&mut self, parts: &[Expr], stop: i64) -> Result<(), Diagnostic> {
        let end = self.new_label();
        let (last, rest) = parts.split_last().expect("a join has parts");
        for part in rest {
            self.expr(part)?;
            self.jump_if_a_is(stop, end);
        }
        self.expr(last)?;
        self.emit(Inst::Mark(end));
        Ok(())
    }

    fn jump_if_a_is(&mut self, value: i64, to: Label) {
        self.emit(Inst::Const(Reg::B, value));
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(Cond::Eq, to));
    }

    /// Puts the text `yes` or `no` in `a`: the truth is in `a`.
    pub fn truth_text(&mut self) {
        let (no, end) = (self.new_label(), self.new_label());
        self.jump_if_a_is(0, no);
        self.text_address(b"yes");
        self.emit(Inst::Jump(end));
        self.emit(Inst::Mark(no));
        self.text_address(b"no");
        self.emit(Inst::Mark(end));
    }

    /// Shows `yes` or `no`: the truth is in `a`.
    pub fn show_truth(&mut self) {
        self.truth_text();
        self.emit(Inst::Push(Reg::A));
        self.call("out_text");
    }
}

impl Lowerer<'_> {
    /// Evaluates `cond` and jumps to `else_to` when it is no.
    pub fn condition(&mut self, cond: &Expr, else_to: Label) -> Result<(), Diagnostic> {
        self.line = cond.line;
        self.emit(Inst::Line(cond.line));
        self.expr(cond)?;
        self.jump_if_a_is(0, else_to);
        Ok(())
    }

    /// `if` with its `otherwise if` branches and the final `otherwise`.
    pub fn if_chain(
        &mut self,
        branches: &[(Expr, Vec<Stmt>)],
        otherwise: Option<&[Stmt]>,
    ) -> Result<(), Diagnostic> {
        let end = self.new_label();
        for (cond, body) in branches {
            let next = self.new_label();
            self.condition(cond, next)?;
            self.stmts(body)?;
            self.emit(Inst::Jump(end));
            self.emit(Inst::Mark(next));
        }
        self.stmts(otherwise.unwrap_or_default())?;
        self.emit(Inst::Mark(end));
        Ok(())
    }
}

impl Lowerer<'_> {
    /// `skip` goes back to the condition, `stop` to the end.
    pub fn repeat_while(&mut self, cond: &Expr, body: &[Stmt]) -> Result<(), Diagnostic> {
        let (top, end) = (self.new_label(), self.new_label());
        self.emit(Inst::Mark(top));
        self.condition(cond, end)?;
        self.loops.push((top, end));
        let done = self.stmts(body);
        self.loops.pop();
        done?;
        self.emit(Inst::Jump(top));
        self.emit(Inst::Mark(end));
        Ok(())
    }
}
