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
    /// A slot whose text is written, cut and with invisible characters spelled, after a
    /// `"` and before the first part.
    shown: Option<u32>,
}

/// `text` with each invisible character written `\u{XXXX}`, the same set and form as the
/// text of failure 5, so the source in a message cannot reorder or hide anything (M2-R30).
fn spell_invisible(text: &str) -> String {
    let spell = |c: char| match crate::chars::unshowable(c) {
        true => format!("\\u{{{:04X}}}", c as u32),
        false => c.to_string(),
    };
    text.chars().map(spell).collect()
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
        let mut parts = parts.map(String::from);
        parts[1] = spell_invisible(&parts[1]);
        let line = self.line;
        self.fails.push(Fail {
            label,
            line,
            code,
            parts,
            value,
            shown: None,
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

    /// Failure 5 for the text in `slot` that `source` (`x as number`) could not read.
    pub fn not_a_number(&mut self, source: &str, slot: u32) -> Label {
        let label = self.fail_label(5, "\" is not a number (", source, ")");
        self.fails.last_mut().expect("just added").shown = Some(slot);
        label
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
            if let Some(slot) = f.shown {
                self.push_text(b"\"");
                self.call("err_text");
                self.emit(Inst::LoadSlot(Reg::A, slot));
                self.emit(Inst::Push(Reg::A));
                self.call("err_shown");
            }
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
