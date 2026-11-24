//! Reading a table file into a list of records.

use super::asm::Asm;
use super::csv_header::{get, load_at, set};
use super::csv_write::count_up;
use crate::ir::Cond::Eq;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

// Slots of `table_record`.
const RECORD: u32 = 1;
const COL: u32 = 2;
const COLS: u32 = 3;
const ITEMS: u32 = 4;
const FIELD: u32 = 5;
const CELL: u32 = 6;
const ENTRY: u32 = 7;
const KIND: u32 = 8;
const VALUE: u32 = 9;
const MAP: u32 = 10;

/// Pushes the word `offset` bytes into the descriptor entry in slot `ENTRY`.
fn push_entry(s: &mut Asm, offset: i32) {
    s.i(LoadSlot(A, ENTRY));
    load_at(s, offset);
    s.i(Push(A));
}

/// `table_record(row, map, cols, desc, shown, line, row_line)`: a record of the descriptor
/// `desc` made from the `cols` cells of `row` (a list of texts). `map` has the field each
/// column names (`csv_header`). A text cell is kept as it is; a number or truth cell goes
/// through `cell_number` or `cell_truth`, which end the program with failure 6 for a bad one,
/// naming `row_line`. Slots: see the constants; `ENTRY` is the field's entry in `desc`.
fn table_record() -> Function {
    let mut s = Asm::default();
    for (slot, arg) in [(COLS, 3), (MAP, 2)] {
        s.i(LoadArg(A, arg));
        s.i(StoreSlot(slot, A));
    }
    s.i(LoadArg(A, 1));
    load_at(&mut s, 16);
    s.i(StoreSlot(ITEMS, A));
    s.i(LoadArg(A, 4));
    load_at(&mut s, 16);
    s.i(Shift(Shift::Left, 3));
    s.i(Push(A));
    s.push_arg(6);
    s.call("alloc");
    s.i(StoreSlot(RECORD, A));
    count_up(&mut s, COL, COLS, |s| {
        let [text, store, truth] = [(); 3].map(|_| s.label());
        get(s, MAP, COL);
        s.i(StoreSlot(FIELD, A));
        get(s, ITEMS, COL);
        s.i(StoreSlot(CELL, A));
        s.i(LoadSlot(A, FIELD));
        s.i(Shift(Shift::Left, 4));
        s.i(LoadArg(B, 4));
        s.i(Int(IntOp::Add));
        s.i(StoreSlot(ENTRY, A));
        s.i(LoadSlot(A, ENTRY));
        load_at(s, 32);
        load_at(s, 0);
        s.i(StoreSlot(KIND, A));
        s.i(LoadSlot(A, CELL));
        s.i(StoreSlot(VALUE, A));
        s.i(LoadSlot(A, KIND));
        s.i(Const(B, 1));
        s.jump_if(Eq, store);
        s.push_slot(CELL);
        push_entry(s, 24);
        for arg in [5, 6, 7] {
            s.push_arg(arg);
        }
        s.i(LoadSlot(A, KIND));
        s.i(Const(B, 2));
        s.jump_if(Eq, truth);
        s.call("cell_number");
        s.i(Jump(text));
        s.i(Mark(truth));
        s.call("cell_truth");
        s.i(Mark(text));
        s.i(StoreSlot(VALUE, A));
        s.i(Mark(store));
        set(s, RECORD, FIELD, VALUE);
    });
    s.i(LoadSlot(A, RECORD));
    s.i(Return);
    s.finish("table_record", 7, 10)
}

pub fn routines() -> Vec<Function> {
    vec![table_record()]
}

#[cfg(test)]
mod tests {
    use super::super::csv_read::tests::with_cursor;
    use crate::ir::Inst::*;
    use crate::ir::{Data, Reg::A};

    /// Reads `input`'s header and first row and makes the record of the row, with the three
    /// fields of `Expense`. Gives the status, what `show` shows of it and the error output.
    fn record(input: &str) -> (i32, String, String) {
        let next = [
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Text(0)),
            Push(A),
        ];
        let mut after = Vec::new();
        for slot in [2, 3] {
            after.extend(next.clone());
            after.extend([
                Const(A, 7),
                Push(A),
                Call("csv_next".into()),
                StoreSlot(slot, A),
            ]);
        }
        after.extend([
            LoadSlot(A, 2),
            Push(A),
            AddressOf(A, Data::Desc(4)),
            Push(A),
        ]);
        after.extend([AddressOf(A, Data::Text(0)), Push(A), Const(A, 7), Push(A)]);
        after.extend([Call("csv_header".into()), StoreSlot(4, A)]);
        after.extend([
            LoadSlot(A, 3),
            Push(A),
            LoadSlot(A, 4),
            Push(A),
            Const(A, 3),
            Push(A),
        ]);
        after.extend([
            AddressOf(A, Data::Desc(4)),
            Push(A),
            AddressOf(A, Data::Text(0)),
        ]);
        after.extend([Push(A), Const(A, 7), Push(A), LoadSlot(A, 1)]);
        after.extend([
            Load {
                to: A,
                base: A,
                offset: 24,
            },
            Push(A),
        ]);
        after.extend([
            Call("table_record".into()),
            Push(A),
            AddressOf(A, Data::Desc(4)),
        ]);
        after.extend([Push(A), Call("show_value".into()), Call("flush".into())]);
        let o = with_cursor(input.as_bytes(), after);
        let text = |bytes| String::from_utf8(bytes).unwrap();
        (o.status, text(o.stdout), text(o.stderr))
    }

    #[test]
    fn a_row_becomes_a_record_by_the_names_in_the_header() {
        let ok = |shown: &str| (0, shown.to_string(), String::new());
        let head = "name,amount,paid\n";
        assert_eq!(
            record(&format!("{head}Lunch,12.5,yes\n")),
            ok("Expense with name \"Lunch\", amount 12.5, paid yes")
        );
        let flipped = record("paid,name,amount\nno,Tea,3\n");
        assert_eq!(flipped, ok("Expense with name \"Tea\", amount 3, paid no"));
        let kept = record(&format!("{head}\"  a \"\"b\"\" \",1,yes"));
        assert_eq!(
            kept.1,
            "Expense with name \"  a \"\"b\"\" \", amount 1, paid yes"
        );
        assert_eq!(
            record(&format!("{head},0,no")).1,
            "Expense with name \"\", amount 0, paid no"
        );
    }
}
