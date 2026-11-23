//! Failure blocks: one per failing instruction, emitted after the function's `Return`.
use super::Lowerer;
use crate::ir::{Inst, Label, Reg};

/// A run-time failure to report: the line, the failure number and the message pieces.
pub(crate) struct Fail {
    label: Label,
    line: u32,
    code: i64,
    parts: [String; 3],
}

impl Lowerer<'_> {
    pub fn new_label(&mut self) -> Label {
        self.labels += 1;
        Label(self.labels - 1)
    }

    /// A label that reports failure `code` at the current line: `<before><source><after>`.
    fn fail_label(&mut self, code: i64, before: &str, source: &str, after: &str) -> Label {
        let label = self.new_label();
        let parts = [before, source, after].map(String::from);
        let line = self.line;
        self.fails.push(Fail {
            label,
            line,
            code,
            parts,
        });
        label
    }

    /// Failure 4, for the expression whose source is `source`.
    pub fn out_of_range(&mut self, source: &str) -> Label {
        self.fail_label(4, "a number went out of range (", source, ")")
    }

    /// Failure 1, for the divisor whose source is `source`.
    pub fn divide_by_zero(&mut self, source: &str) -> Label {
        self.fail_label(1, "dividing by zero (", source, " is 0)")
    }

    /// Emits the failure blocks collected so far.
    pub fn emit_fails(&mut self) {
        for f in std::mem::take(&mut self.fails) {
            self.emit(Inst::Mark(f.label));
            self.emit(Inst::Const(Reg::A, f.line as i64));
            self.emit(Inst::Push(Reg::A));
            self.emit(Inst::Const(Reg::A, f.code));
            self.emit(Inst::Push(Reg::A));
            self.call("fail_begin");
            for part in &f.parts {
                self.push_text(part.as_bytes());
                self.call("err_text");
            }
            self.call("fail_end");
        }
    }
}
