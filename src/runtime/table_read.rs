//! Reading a table file into a list of records.

use super::asm::Asm;
use super::csv_header::{get, load_at, set};
use super::csv_write::count_up;
use crate::ir::Cond::{Eq, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{CWD, Function, Inst::*, IntOp, Shift};

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

const LIST: u32 = 1; // slots of `table_read`
const CURSOR: u32 = 2;
const HEADER: u32 = 3;
const HEADER_MAP: u32 = 4;
const WIDTH: u32 = 5;
const ROW: u32 = 6;

/// The next row of the cursor in slot `CURSOR` into `a`, or 0 at the end.
fn next_row(s: &mut Asm) {
    s.push_slot(CURSOR);
    s.push_arg(2);
    s.push_arg(4);
    s.call("csv_next");
}

/// `table_read(path, shown, desc, line)`: the rows of the table file `path` (a NUL-terminated
/// text in the current folder) as a new list of records of `desc`; `shown` names the file in
/// messages. A missing, empty or header-only file gives an empty list; a row with other than
/// the header's number of cells is failure 6. Nothing is shared until the caller stores it.
fn table_read() -> Function {
    let mut s = Asm::default();
    let [rows, done, wrong] = [(); 3].map(|_| s.label());
    s.push_const(4);
    s.push_arg(4);
    s.call("list_new");
    s.i(StoreSlot(LIST, A));
    s.push_const(CWD);
    for arg in [1, 2, 4] {
        s.push_arg(arg);
    }
    s.call("file_bytes");
    s.i(Const(B, 0));
    s.jump_if(Eq, done);
    s.i(Push(A));
    s.push_arg(4);
    s.call("csv_cursor");
    s.i(StoreSlot(CURSOR, A));
    next_row(&mut s);
    s.i(StoreSlot(HEADER, A));
    s.i(Const(B, 0));
    s.jump_if(Eq, done);
    s.push_slot(HEADER);
    for arg in [3, 2, 4] {
        s.push_arg(arg);
    }
    s.call("csv_header");
    s.i(StoreSlot(HEADER_MAP, A));
    s.i(LoadSlot(A, HEADER));
    load_at(&mut s, 0);
    s.i(StoreSlot(WIDTH, A));
    s.i(Mark(rows));
    next_row(&mut s);
    s.i(StoreSlot(ROW, A));
    s.i(Const(B, 0));
    s.jump_if(Eq, done);
    load_at(&mut s, 0);
    s.i(LoadSlot(B, WIDTH));
    s.jump_if(Ne, wrong);
    s.push_slot(ROW);
    s.push_slot(HEADER_MAP);
    s.push_slot(WIDTH);
    for arg in [3, 2, 4] {
        s.push_arg(arg);
    }
    s.i(LoadSlot(A, CURSOR));
    load_at(&mut s, 24);
    s.i(Push(A));
    s.call("table_record");
    s.i(Move { to: B, from: A });
    s.push_slot(LIST);
    s.i(Push(B));
    s.push_arg(4);
    s.call("list_push");
    s.i(Jump(rows));
    s.i(Mark(done));
    s.i(LoadSlot(A, LIST));
    s.i(Return);
    s.i(Mark(wrong));
    s.push_arg(4);
    s.push_arg(2);
    s.i(LoadSlot(A, CURSOR));
    load_at(&mut s, 24);
    s.i(Push(A));
    s.call("fail_row");
    s.push_text(super::HAS);
    s.call("err_text");
    s.i(LoadSlot(A, ROW));
    load_at(&mut s, 0);
    s.i(Push(A));
    s.call("err_int");
    s.push_text(super::CELLS_HEADER);
    s.call("err_text");
    s.push_slot(WIDTH);
    s.call("err_int");
    s.call("fail_end");
    s.finish("table_read", 4, 6)
}

pub fn routines() -> Vec<Function> {
    vec![table_record(), table_read()]
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

    #[test]
    fn a_bad_cell_ends_with_failure_6_on_the_line_of_the_row_and_the_first_one_is_named() {
        let at = "line 7: failure 6: e.csv line 2: ";
        let head = "name,amount,paid\n";
        let cases = [
            ("x,1x,yes", "amount \"1x\" is not a number"),
            ("x,,yes", "amount is empty"),
            ("x,1,maybe", "paid \"maybe\" is not yes or no"),
            ("x,bad,maybe", "amount \"bad\" is not a number"),
        ];
        for (row, want) in cases {
            let (status, shown, err) = record(&format!("{head}{row}\n"));
            assert_eq!(
                (status, shown.as_str(), err),
                (2, "", format!("{at}{want}\n"))
            );
        }
    }
}

#[cfg(test)]
mod file_tests {
    use super::super::tests::program;
    use crate::ir::Inst::*;
    use crate::ir::{Data, Desc, Module, Reg::A};
    use crate::irsim::{self, Faults, Files, Outcome};

    /// Loads the table file `e.csv` of `Expense` records and shows the list. Failures call
    /// the file `shown`.
    fn load_files(mut files: Files) -> (Outcome, String, String) {
        let mut descs = Module::default();
        let (text, number, truth) = (
            descs.desc(Desc::Text),
            descs.desc(Desc::Number),
            descs.desc(Desc::Truth),
        );
        let fields = ["name", "amount", "paid"].map(String::from);
        let fields = fields.into_iter().zip([text, number, truth]).collect();
        let record = descs.desc(Desc::Record {
            name: "Expense".into(),
            fields,
        });
        let list = descs.desc(Desc::List(record));
        let mut body = vec![
            AddressOf(A, Data::Text(0)),
            Push(A),
            AddressOf(A, Data::Text(1)),
        ];
        body.extend([
            Push(A),
            AddressOf(A, Data::Desc(record)),
            Push(A),
            Const(A, 7),
            Push(A),
        ]);
        body.extend([
            Call("table_read".into()),
            Push(A),
            AddressOf(A, Data::Desc(list)),
            Push(A),
        ]);
        body.extend([Call("show_value".into()), Call("flush".into())]);
        let mut m = program(body, &[b"e.csv\0", b"data/e.csv"]);
        m.descs = descs.descs;
        let o = irsim::run_with(&m, "_start", b"", &[], Faults::default(), &mut files);
        let text = |bytes: &[u8]| String::from_utf8(bytes.to_vec()).unwrap();
        let (shown, err) = (text(&o.stdout), text(&o.stderr));
        (o, shown, err)
    }

    fn load(content: Option<&str>) -> (Outcome, String, String) {
        let mut files = Files::default();
        if let Some(content) = content {
            files
                .files
                .insert("e.csv".into(), content.as_bytes().to_vec());
        }
        load_files(files)
    }

    #[test]
    fn the_rows_of_a_file_become_a_list_of_records() {
        let want = "list of (Expense with name \"A\", amount 1, paid yes), \
                    (Expense with name \"B, c\", amount -2.5, paid no)";
        for content in [
            "name,amount,paid\nA,1,yes\n\"B, c\",-2.5,no\n",
            "paid,name,amount\r\nyes,A,1\r\nno,\"B, c\",-2.5",
        ] {
            let (o, shown, err) = load(Some(content));
            assert_eq!(
                (o.status, shown.as_str(), err.as_str()),
                (0, want, ""),
                "for {content:?}"
            );
        }
    }

    #[test]
    fn a_missing_empty_or_header_only_file_is_an_empty_table() {
        let none = "empty list of Expense";
        let (o, shown, err) = load(None);
        let notice = "data/e.csv not found; starting with an empty table\n";
        assert_eq!((o.status, shown.as_str(), err.as_str()), (0, none, notice));
        for content in ["", "\n", "\r\n", "name,amount,paid", "name,amount,paid\n\n"] {
            let (o, shown, err) = load(Some(content));
            assert_eq!(
                (o.status, shown.as_str(), err.as_str()),
                (0, none, ""),
                "for {content:?}"
            );
        }
    }

    /// What a file that cannot be loaded makes the program write, and that it shows nothing.
    fn refused(content: &str) -> String {
        let (o, shown, err) = load(Some(content));
        assert_eq!((o.status, shown.as_str()), (2, ""), "for {content:?}");
        err.strip_prefix("line 7: failure 6: data/e.csv line ")
            .unwrap()
            .to_string()
    }

    #[test]
    fn a_row_with_the_wrong_number_of_cells_is_named_with_both_counts() {
        let head = "name,amount,paid\n";
        assert_eq!(
            refused(&format!("{head}A,1,yes\nB,2\n")),
            "3: has 2 cells; the header has 3\n"
        );
        assert_eq!(
            refused(&format!("{head}A,1,yes,x\n")),
            "2: has 4 cells; the header has 3\n"
        );
        assert_eq!(
            refused(&format!("{head}\nA,1,yes\n")),
            "2: has 1 cells; the header has 3\n"
        );
    }

    #[test]
    fn a_bad_third_row_after_two_good_ones_names_its_own_line_and_shows_nothing() {
        let head = "name,amount,paid\nA,1,yes\nB,2,no\n";
        assert_eq!(
            refused(&format!("{head}C,x,yes\nD,4,no\n")),
            "4: amount \"x\" is not a number\n"
        );
        // A quoted cell that spans lines moves the next row's line down with it.
        let spans = "name,amount,paid\n\"A\nB\",1,yes\nC,2,maybe\n";
        assert_eq!(refused(spans), "4: paid \"maybe\" is not yes or no\n");
        let long = format!(
            "name,amount,paid\nA,{},yes\n",
            "9".repeat(3) + &"x".repeat(60)
        );
        let cut = format!(
            "2: amount \"999{}\u{2026}\" is not a number\n",
            "x".repeat(37)
        );
        assert_eq!(refused(&long), cut);
    }

    #[test]
    fn a_table_that_is_a_symbolic_link_is_failure_6() {
        let mut files = Files::default();
        files
            .files
            .insert("e.csv".into(), b"name,amount,paid\n".to_vec());
        files.links.insert("e.csv".into());
        let (o, shown, err) = load_files(files);
        let link = "line 7: failure 6: data/e.csv is reached through a symbolic link\n";
        assert_eq!((o.status, shown.as_str(), err.as_str()), (2, "", link));
    }
}
