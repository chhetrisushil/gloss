//! Lowering expressions: the value ends up in register `a`.
use super::Lowerer;
use crate::ast::print;
use crate::ast::{ArithOp, Expr, ExprKind, Fact};
use crate::check::NameKind;
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
            ExprKind::Paren(inner) => self.expr(inner),
            ExprKind::Name(name) => self.name(e, name),
            ExprKind::Negate(inner) => {
                self.expr(inner)?;
                let range = self.out_of_range(&print::expr(e));
                self.emit(Inst::NumNeg { range });
                Ok(())
            }
            ExprKind::Fact {
                fact: Fact::Whole,
                of,
            } => {
                self.expr(of)?;
                self.emit(Inst::NumWhole);
                Ok(())
            }
            ExprKind::Binary { op, left, right } => {
                self.operands(left, right)?;
                let inst = match op {
                    ArithOp::Add => Inst::NumAdd {
                        range: self.out_of_range(&print::expr(e)),
                    },
                    ArithOp::Sub => Inst::NumSub {
                        range: self.out_of_range(&print::expr(e)),
                    },
                    ArithOp::Mul => Inst::NumMul {
                        range: self.out_of_range(&print::expr(e)),
                    },
                    ArithOp::Div => Inst::NumDiv {
                        zero: self.divide_by_zero(&print::expr(right)),
                        range: self.out_of_range(&print::expr(e)),
                    },
                };
                self.emit(inst);
                Ok(())
            }
            ExprKind::Remainder { of, by } => {
                self.operands(of, by)?;
                let zero = self.divide_by_zero(&print::expr(by));
                self.emit(Inst::NumRem { zero });
                Ok(())
            }
            _ => Err(Diagnostic::error(
                self.src,
                e.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }

    /// Left in `a`, right in `b`.
    fn operands(&mut self, left: &Expr, right: &Expr) -> Result<(), Diagnostic> {
        self.expr(left)?;
        self.emit(Inst::Push(Reg::A));
        self.expr(right)?;
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::Pop(Reg::A));
        Ok(())
    }

    fn name(&mut self, e: &Expr, name: &str) -> Result<(), Diagnostic> {
        match self.types.names.get(&e.id) {
            Some(NameKind::Variable | NameKind::LoopVariable) => {
                self.emit(Inst::LoadSlot(Reg::A, self.slots[name]));
                Ok(())
            }
            Some(NameKind::Input) => {
                let at = self.inputs.iter().position(|i| i == name).unwrap_or(0);
                self.emit(Inst::LoadArg(Reg::A, at as u32 + 1));
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
