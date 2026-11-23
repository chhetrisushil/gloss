//! Failure blocks: one per failing instruction, emitted after the function's `Return`.
use super::Lowerer;
use crate::ir::{Inst, Label, Reg};

/// A run-time failure to report: the line, the failure number and the message pieces.
pub(crate) struct Fail {
    label: Label,
    line: u32,
    code: i64,
    parts: [String; 3],
    /// A slot whose number is written between the source and the last part, after ` is `.
    value: Option<u32>,
}

impl Lowerer<'_> {
    pub fn new_label(&mut self) -> Label {
        self.labels += 1;
        Label(self.labels - 1)
    }

    /// A label that reports failure `code` at the current line: `<before><source><after>`.
    fn fail_label(&mut self, code: i64, before: &str, source: &str, after: &str) -> Label {
        self.fail_label_with(code, [before, source, after], None)
    }

    fn fail_label_with(&mut self, code: i64, parts: [&str; 3], value: Option<u32>) -> Label {
        let label = self.new_label();
        let parts = parts.map(String::from);
        let line = self.line;
        self.fails.push(Fail {
            label,
            line,
            code,
            parts,
            value,
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

    /// Failure 10, for a call of `feature` that ran out of stack.
    pub fn too_deep(&mut self, feature: &str) -> Label {
        self.fail_label(10, "calls nested too deep (", feature, ")")
    }

    /// Failure 3 for a count end or start whose source is `source` and whose value is in
    /// `slot`; `what` is `end` or `start`.
    pub fn not_whole(&mut self, what: &str, source: &str, slot: u32) -> Label {
        let before = format!("the {what} of a count must be whole (");
        self.fail_label_with(3, [&before, source, ")"], Some(slot))
    }

    /// The end of a feature that gives a value, which the checker guarantees no path reaches
    /// (plan ruling P2). If it is reached anyway, flush standard output, write
    /// `internal error: feature <name> ended without giving a value` and a newline (no
    /// `line N:` prefix: it is not a language failure, and the spec has no row for it) and
    /// exit 2.
    pub fn fall_off_end(&mut self, feature: &str) {
        self.call("flush");
        let message = format!("internal error: feature {feature} ended without giving a value");
        self.push_text(message.as_bytes());
        self.call("err_text");
        self.call("fail_end");
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
            for (i, part) in f.parts.iter().enumerate() {
                if let (2, Some(slot)) = (i, f.value) {
                    self.push_text(b" is ");
                    self.call("err_text");
                    self.emit(Inst::LoadSlot(Reg::A, slot));
                    self.emit(Inst::Push(Reg::A));
                    self.call("err_number");
                }
                self.push_text(part.as_bytes());
                self.call("err_text");
            }
            self.call("fail_end");
        }
    }
}
