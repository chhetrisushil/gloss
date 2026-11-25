//! Lowering expressions: the value ends up in register `a`.
use super::{Lowerer, Mode};
use crate::ast::print;
use crate::ast::{ArithOp, Conversion, Expr, ExprKind, Fact};
use crate::check::NameKind;
use crate::diag::Diagnostic;
use crate::ir::{Cond, Data, Global, Inst, Reg};

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
            ExprKind::Text(text) => {
                self.text_address(text.as_bytes());
                Ok(())
            }
            ExprKind::Convert {
                value,
                to: Conversion::ToText,
            } => {
                self.expr(value)?;
                if self.is_truth(value) {
                    self.truth_text();
                } else {
                    self.emit(Inst::Push(Reg::A));
                    self.emit(Inst::Const(Reg::A, i64::from(self.line)));
                    self.emit(Inst::Push(Reg::A));
                    self.call("number_text");
                }
                Ok(())
            }
            ExprKind::Convert {
                value,
                to: Conversion::ToTextPlaces(places),
            } => {
                self.expr(value)?;
                self.emit(Inst::Push(Reg::A));
                self.emit(Inst::Const(Reg::A, i64::from(*places)));
                self.emit(Inst::Push(Reg::A));
                self.emit(Inst::Const(Reg::A, i64::from(self.line)));
                self.emit(Inst::Push(Reg::A));
                self.call("number_places");
                Ok(())
            }
            ExprKind::Convert {
                value,
                to: Conversion::ToNumber,
            } => {
                self.expr(value)?;
                let text = self.new_temp();
                let fail = self.not_a_number(&print::expr(e), text);
                self.emit(Inst::StoreSlot(text, Reg::A));
                self.emit(Inst::Push(Reg::A));
                self.call("parse_number");
                self.emit(Inst::Const(Reg::B, 0));
                self.emit(Inst::Compare);
                self.emit(Inst::JumpIf(Cond::Ne, fail));
                self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::NumBuf)));
                self.emit(Inst::Load {
                    to: Reg::A,
                    base: Reg::A,
                    offset: 0,
                });
                Ok(())
            }
            ExprKind::Paren(inner) => self.expr(inner),
            ExprKind::BuildRecord { record, fields } => self.build_record(record, fields),
            ExprKind::FieldOf { field, of } => self.field_of(field, of),
            ExprKind::Call { feature, inputs } => self.feature_call(feature, inputs),
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
            ExprKind::Fact {
                fact: Fact::Count,
                of,
            } => self.list_count(of),
            ExprKind::Fact {
                fact: fact @ (Fact::First | Fact::Last),
                of,
            } => self.list_end(*fact, of),
            ExprKind::ItemOf { position, of } => self.item_of(position, of),
            ExprKind::EmptyList(_) => self.build_list(&[]),
            ExprKind::BuildList(items) => self.build_list(items),
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
            ExprKind::Shown if self.mode == Mode::Test => {
                self.emit(Inst::Const(Reg::A, i64::from(self.line)));
                self.emit(Inst::Push(Reg::A));
                self.call("shown_list");
                Ok(())
            }
            ExprKind::Remainder { of, by } => {
                self.operands(of, by)?;
                let zero = self.divide_by_zero(&print::expr(by));
                self.emit(Inst::NumRem { zero });
                Ok(())
            }
            _ => self.truth(e).unwrap_or_else(|| {
                Err(Diagnostic::error(
                    self.src,
                    e.line,
                    "this compiler cannot build this expression yet",
                ))
            }),
        }
    }

    /// Left in `a`, right in `b`.
    pub fn operands(&mut self, left: &Expr, right: &Expr) -> Result<(), Diagnostic> {
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
            Some(NameKind::Table) => {
                let index = self.table_index(name).expect("the checker found the table");
                self.load_table(index);
                Ok(())
            }
            Some(NameKind::FeatureCall) => self.feature_call(name, &[]),
            _ => Err(Diagnostic::error(
                self.src,
                e.line,
                "this compiler cannot build this expression yet",
            )),
        }
    }

    /// Pushes the inputs left to right, checks the stack, calls; the backend drops the inputs
    /// and the result is in `a`. The check's size is filled in once every feature is lowered.
    /// A trace build evaluates the inputs into hidden slots first, so no user code runs while
    /// the `enter` event is open, and writes `enter` after the stack check (failure 10 writes
    /// none) and `leave` after the call.
    pub fn feature_call(&mut self, feature: &str, inputs: &[Expr]) -> Result<(), Diagnostic> {
        let mut held = Vec::new();
        for input in inputs {
            self.expr(input)?;
            if self.mode == Mode::Trace {
                let slot = self.new_temp();
                self.emit(Inst::StoreSlot(slot, Reg::A));
                held.push(slot);
            } else {
                self.emit(Inst::Push(Reg::A));
            }
        }
        let fail = self.too_deep(feature);
        self.emit(Inst::StackCheck { bytes: 0, fail });
        if self.mode == Mode::Trace {
            self.enter_event(feature, &held);
        }
        self.emit(Inst::Call(super::function_name(feature)));
        self.restore_line();
        if self.mode == Mode::Trace {
            self.leave_event(feature);
        }
        Ok(())
    }
}
