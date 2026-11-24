//! Turning a cell of a table file into the value of a field.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst::*, IntOp};

/// Starts failure 6 for the cell (`fail_row`), then writes the field's name. Arguments of the
/// routines: the cell, the field's name, `shown`, `line` and `row`, the row's first line.
fn begin(s: &mut Asm) {
    s.push_arg(4);
    s.push_arg(3);
    s.push_arg(5);
    s.call("fail_row");
    s.push_arg(2);
    s.call("err_text");
}

/// Ends the failure with ` "<cell>` and `end`, which opens with the closing quote.
fn quoted(s: &mut Asm, end: u32) {
    s.push_text(super::SPACE_QUOTE);
    s.call("err_text");
    s.push_arg(1);
    s.call("err_shown");
    s.push_text(end);
    s.call("err_text");
    s.call("fail_end");
}

/// `cell_number(cell, name, shown, line, row)`: the number in the cell, as `parse_number`
/// reads it (spaces at both ends are ignored). A cell with nothing else in it is failure 6
/// `<name> is empty`; any other that is not a number is `<name> "<cell>" is not a number`,
/// the cell as `err_shown` writes it. Slots: the index, the length.
fn cell_number() -> Function {
    let mut s = Asm::default();
    let [bad, scan, empty, other] = [(); 4].map(|_| s.label());
    s.push_arg(1);
    s.call("parse_number");
    s.i(Const(B, 0));
    s.jump_if(Ne, bad);
    s.i(AddressOf(A, Data::Global(Global::NumBuf)));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Return);
    s.i(Mark(bad));
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(2, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(scan));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, empty);
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Const(B, i64::from(b' ')));
    s.jump_if(Ne, other);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(scan));
    s.i(Mark(empty));
    begin(&mut s);
    s.push_text(super::IS_EMPTY);
    s.call("err_text");
    s.call("fail_end");
    s.i(Mark(other));
    begin(&mut s);
    quoted(&mut s, super::NOT_NUMBER);
    s.finish("cell_number", 5, 2)
}

/// `cell_truth(cell, name, shown, line, row)`: 1 for exactly `yes`, 0 for exactly `no`; any
/// other cell is failure 6, `<name> "<cell>" is not yes or no`.
fn cell_truth() -> Function {
    let mut s = Asm::default();
    for (word, value) in [(super::YES, 1), (super::NO, 0)] {
        let other = s.label();
        s.push_arg(1);
        s.push_text(word);
        s.call("text_equal");
        s.i(Const(B, 0));
        s.jump_if(Eq, other);
        s.i(Const(A, value));
        s.i(Return);
        s.i(Mark(other));
    }
    begin(&mut s);
    quoted(&mut s, super::NOT_YES_NO);
    s.finish("cell_truth", 5, 0)
}

pub fn routines() -> Vec<Function> {
    vec![cell_number(), cell_truth()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::{Data, Inst::*, Module, Reg::A};

    /// Calls the routine `name` on the cell `cell` of the field `amount` of `e.csv`, a row that
    /// began on line 9, from line 7. Gives the status and standard error, where a value that
    /// comes back is written as an integer.
    pub fn convert(routine: &str, cell: &str) -> (i32, String) {
        let texts = [cell, "amount", "e.csv"];
        let mut probe = Module::default();
        let at = texts.map(|t| probe.text(t.as_bytes()));
        let mut body = Vec::new();
        for i in at {
            body.extend([AddressOf(A, Data::Text(i)), Push(A)]);
        }
        body.extend([Const(A, 7), Push(A), Const(A, 9), Push(A)]);
        body.extend([Call(routine.into()), Push(A), Call("err_int".into())]);
        let raw: Vec<&[u8]> = texts.iter().map(|t| t.as_bytes()).collect();
        let o = run(&program(body, &raw));
        (o.status, String::from_utf8(o.stderr).unwrap())
    }

    fn refused(cell: &str) -> String {
        let (status, err) = convert("cell_number", cell);
        assert_eq!(status, 2, "for {cell:?}");
        let head = "line 7: failure 6: e.csv line 9: amount ";
        err.strip_prefix(head).expect(&err).to_string()
    }

    #[test]
    fn a_truth_cell_is_exactly_yes_or_no() {
        assert_eq!(convert("cell_truth", "yes"), (0, "1".to_string()));
        assert_eq!(convert("cell_truth", "no"), (0, "0".to_string()));
        let line = "line 7: failure 6: e.csv line 9: amount";
        for cell in ["maybe", "Yes", "NO", "yes ", " no", "y", "", "1", "true"] {
            let want = format!("{line} \"{cell}\" is not yes or no\n");
            assert_eq!(convert("cell_truth", cell), (2, want), "for {cell:?}");
        }
        let long = "y".repeat(50);
        let want = format!("{line} \"{}\u{2026}\" is not yes or no\n", "y".repeat(40));
        assert_eq!(convert("cell_truth", &long), (2, want));
    }

    #[test]
    fn a_number_cell_is_read_as_parse_number_reads_it() {
        for (cell, want) in [
            ("12", 120_000),
            (" 7 ", 70_000),
            ("-1.5", -15_000),
            ("0.0001", 1),
        ] {
            assert_eq!(convert("cell_number", cell), (0, want.to_string()));
        }
    }

    #[test]
    fn a_cell_that_is_not_a_number_is_named_and_an_empty_one_says_so() {
        assert_eq!(refused("12,50"), "\"12,50\" is not a number\n");
        assert_eq!(refused("abc"), "\"abc\" is not a number\n");
        assert_eq!(refused("1 2"), "\"1 2\" is not a number\n");
        assert_eq!(refused(""), "is empty\n");
        assert_eq!(refused("   "), "is empty\n");
        assert_eq!(refused("\t"), "\"\\u{0009}\" is not a number\n");
        assert_eq!(refused("a\"b"), "\"a\"\"b\" is not a number\n");
        let long = format!("x{}", "9".repeat(60));
        let want = format!("\"x{}\u{2026}\" is not a number\n", "9".repeat(39));
        assert_eq!(refused(&long), want);
    }
}
