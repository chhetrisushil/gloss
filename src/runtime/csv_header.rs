//! Matching the header row of a table file to the fields of its record.

use super::asm::Asm;
use super::csv_write::count_up;
use crate::ir::Cond::{Eq, Lt};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

// Slots of `csv_header`.
const MAP: u32 = 1;
const COLS: u32 = 2;
const FIELDS: u32 = 3;
const I: u32 = 4;
const J: u32 = 5;
const ITEMS: u32 = 6;
const CELL: u32 = 7;
const FOUND: u32 = 8;

/// `a = word at a + offset`.
fn load_at(s: &mut Asm, offset: i32) {
    s.i(Load {
        to: A,
        base: A,
        offset,
    });
}

/// The address of word `index` (a slot) of the block in slot `base`, into `a`.
fn word_at(s: &mut Asm, base: u32, index: u32) {
    s.i(LoadSlot(A, index));
    s.i(Shift(Shift::Left, 3));
    s.i(LoadSlot(B, base));
    s.i(Int(IntOp::Add));
}

/// The word `index` of the block in slot `base`, into `a`.
fn get(s: &mut Asm, base: u32, index: u32) {
    word_at(s, base, index);
    load_at(s, 0);
}

/// Sets that word to the value in slot `value`.
fn set(s: &mut Asm, base: u32, index: u32, value: u32) {
    word_at(s, base, index);
    s.i(LoadSlot(B, value));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
}

/// The name text of the field in slot `J` of the record descriptor (argument 2), into `a`.
fn field_name(s: &mut Asm) {
    s.i(LoadSlot(A, J));
    s.i(Shift(Shift::Left, 4));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    load_at(s, 24);
}

/// Starts failure 6: `<shown> line 1: column `.
fn column(s: &mut Asm) {
    s.push_arg(4);
    s.push_arg(3);
    s.push_const(1);
    s.call("fail_row");
    s.push_text(super::COLUMN);
    s.call("err_text");
}

/// `csv_header(cells, desc, shown, line)`: for the header row `cells` (a list of texts) and the
/// record descriptor `desc`, a block of one word per column holding the index of the field
/// that column names. A column that is not a field is failure 6 (`column amout is not a field
/// of Expense`). `I` is the column, `J` a field, `FOUND` the field a column names.
fn csv_header() -> Function {
    let mut s = Asm::default();
    let unknown = s.label();
    for (slot, offset) in [(COLS, 0), (ITEMS, 16)] {
        s.i(LoadArg(A, 1));
        load_at(&mut s, offset);
        s.i(StoreSlot(slot, A));
    }
    s.i(LoadArg(A, 2));
    load_at(&mut s, 16);
    s.i(StoreSlot(FIELDS, A));
    s.i(LoadSlot(A, COLS));
    s.i(Shift(Shift::Left, 3));
    s.i(Push(A));
    s.push_arg(4);
    s.call("alloc");
    s.i(StoreSlot(MAP, A));
    count_up(&mut s, I, COLS, |s| {
        get(s, ITEMS, I);
        s.i(StoreSlot(CELL, A));
        s.i(Const(A, -1));
        s.i(StoreSlot(FOUND, A));
        count_up(s, J, FIELDS, |s| {
            let other = s.label();
            s.push_slot(CELL);
            field_name(s);
            s.i(Push(A));
            s.call("text_equal");
            s.i(Const(B, 0));
            s.jump_if(Eq, other);
            s.i(LoadSlot(A, J));
            s.i(StoreSlot(FOUND, A));
            s.i(Mark(other));
        });
        s.i(LoadSlot(A, FOUND));
        s.i(Const(B, 0));
        s.jump_if(Lt, unknown);
        set(s, MAP, I, FOUND);
    });
    s.i(LoadSlot(A, MAP));
    s.i(Return);
    s.i(Mark(unknown));
    column(&mut s);
    s.push_slot(CELL);
    s.call("err_shown");
    s.push_text(super::NOT_A_FIELD);
    s.call("err_text");
    s.i(LoadArg(A, 2));
    load_at(&mut s, 8);
    s.i(Push(A));
    s.call("err_text");
    s.call("fail_end");
    s.finish("csv_header", 4, 8)
}

pub fn routines() -> Vec<Function> {
    vec![csv_header()]
}

#[cfg(test)]
mod tests {
    use super::super::csv_read::tests::with_cursor;
    use crate::ir::Inst::*;
    use crate::ir::{Data, Reg::A};

    /// Matches the header `input` to `Expense`, then shows the field of each of `cols` columns.
    fn header(input: &str, cols: i32) -> (i32, String) {
        let mut after = vec![
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Text(0)),
            Push(A),
        ];
        after.extend([Const(A, 7), Push(A), Call("csv_next".into()), Push(A)]);
        after.extend([
            AddressOf(A, Data::Desc(4)),
            Push(A),
            AddressOf(A, Data::Text(0)),
        ]);
        after.extend([Push(A), Const(A, 7), Push(A), Call("csv_header".into())]);
        after.push(StoreSlot(1, A));
        for column in 0..cols {
            let word = Load {
                to: A,
                base: A,
                offset: 8 * column,
            };
            after.extend([LoadSlot(A, 1), word, Push(A), Call("err_int".into())]);
            after.extend([
                AddressOf(A, Data::Text(2)),
                Push(A),
                Call("err_text".into()),
            ]);
        }
        let o = with_cursor(input.as_bytes(), after);
        (o.status, String::from_utf8(o.stderr).unwrap())
    }

    #[test]
    fn columns_are_matched_to_fields_by_name_in_any_order() {
        assert_eq!(header("name,amount,paid", 3), (0, "0 1 2 ".into()));
        assert_eq!(header("paid,name,amount", 3), (0, "2 0 1 ".into()));
    }

    #[test]
    fn a_column_that_is_not_a_field_is_named_as_failure_6_on_line_1() {
        let err = |cell: &str| {
            let tail = format!("{cell} is not a field of Expense\n");
            (2, format!("line 7: failure 6: e.csv line 1: column {tail}"))
        };
        assert_eq!(header("name,amout,paid", 3), err("amout"));
        assert_eq!(header("Name", 1), err("Name"));
        assert_eq!(header("name,,paid", 3), err(""));
    }
}
