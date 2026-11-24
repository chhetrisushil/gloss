//! Writing a table as CSV text: the header, then one line per record.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Lt, Ne};
use crate::ir::Reg::{self, A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

fn load(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(Load { to, base, offset });
}

fn store(s: &mut Asm, base: Reg, offset: i32, from: Reg) {
    s.i(Store { base, offset, from });
}

fn load_byte(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(LoadByte { to, base, offset });
}

fn store_byte(s: &mut Asm, base: Reg, offset: i32, from: Reg) {
    s.i(StoreByte { base, offset, from });
}

/// The bytes a cell must be quoted for.
const SPECIAL: [u8; 4] = *b",\"\r\n";

/// Pushes the word at `offset` of the builder (argument 1).
fn push_builder(s: &mut Asm, offset: i32) {
    s.i(LoadArg(A, 1));
    load(s, A, A, offset);
    s.i(Push(A));
}

/// `csv_byte(b, byte, line)`: appends one byte to the builder `b`, a block of three words:
/// the text being built, its length so far, and its room. A full text first moves to one of
/// twice the room (`grow_text`); its length word is only set by `csv_text`, at the end.
fn csv_byte() -> Function {
    let mut s = Asm::default();
    let ready = s.label();
    s.i(LoadArg(B, 1));
    load(&mut s, A, B, 8);
    load(&mut s, B, B, 16);
    s.jump_if(Lt, ready);
    for offset in [0, 8, 16] {
        push_builder(&mut s, offset);
    }
    s.push_arg(3);
    s.call("grow_text");
    s.i(LoadArg(B, 1));
    store(&mut s, B, 0, A);
    load(&mut s, A, B, 16);
    s.i(Shift(Shift::Left, 1));
    store(&mut s, B, 16, A);
    s.i(Mark(ready));
    s.i(LoadArg(B, 1));
    load(&mut s, A, B, 8);
    load(&mut s, B, B, 0);
    s.i(Int(IntOp::Add));
    s.i(LoadArg(B, 2));
    store_byte(&mut s, A, 8, B);
    s.i(LoadArg(B, 1));
    load(&mut s, A, B, 8);
    s.i(AddImm(A, 1));
    store(&mut s, B, 8, A);
    s.finish("csv_byte", 3, 0)
}

/// The byte of the text (argument 2) at the index in slot 1, into `a`.
fn text_byte(s: &mut Asm) {
    s.i(LoadArg(A, 2));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    load_byte(s, A, A, 8);
}

/// Calls `csv_byte` with the constant `byte`.
fn put_const(s: &mut Asm, byte: u8) {
    s.push_arg(1);
    s.push_const(i64::from(byte));
    s.push_arg(3);
    s.call("csv_byte");
}

/// Calls `csv_byte` with the byte in `a`.
fn put_a(s: &mut Asm) {
    s.i(Move { to: B, from: A });
    s.push_arg(1);
    s.i(Push(B));
    s.push_arg(3);
    s.call("csv_byte");
}

/// Runs `each` once per byte of the text (argument 2), with slot 1 the index.
fn per_byte(s: &mut Asm, each: impl FnOnce(&mut Asm)) {
    let (again, done) = (s.label(), s.label());
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadArg(B, 2));
    load(s, B, B, 0);
    s.jump_if(Ge, done);
    each(s);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
}

/// `csv_add(b, text, line)`: appends the bytes of `text` as they are. Slot 1: the index.
fn csv_add() -> Function {
    let mut s = Asm::default();
    per_byte(&mut s, |s| {
        text_byte(s);
        put_a(s);
    });
    s.finish("csv_add", 3, 1)
}

/// `csv_cell(b, text, line)`: appends a text cell. One that holds a comma, a quote or a line
/// break is wrapped in quotes with each quote doubled. Slots: 1 the index, 2 whether to quote.
fn csv_cell() -> Function {
    let mut s = Asm::default();
    let (unquoted, single, end) = (s.label(), s.label(), s.label());
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    per_byte(&mut s, |s| {
        let (hit, next) = (s.label(), s.label());
        for special in SPECIAL {
            text_byte(s);
            s.i(Const(B, i64::from(special)));
            s.jump_if(Eq, hit);
        }
        s.i(Jump(next));
        s.i(Mark(hit));
        s.i(Const(A, 1));
        s.i(StoreSlot(2, A));
        s.i(Mark(next));
    });
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Eq, unquoted);
    put_const(&mut s, b'"');
    s.i(Mark(unquoted));
    per_byte(&mut s, |s| {
        text_byte(s);
        s.i(Const(B, i64::from(b'"')));
        s.jump_if(Ne, single);
        put_const(s, b'"');
        s.i(Mark(single));
        text_byte(s);
        put_a(s);
    });
    s.i(LoadSlot(A, 2));
    s.i(Const(B, 0));
    s.jump_if(Eq, end);
    put_const(&mut s, b'"');
    s.i(Mark(end));
    s.finish("csv_cell", 3, 2)
}

// Slots of `csv_text`.
const BUILDER: u32 = 1;
const FIELDS: u32 = 2;
const FIELD: u32 = 3;
const ROWS: u32 = 4;
const ROW: u32 = 5;
const RECORD: u32 = 6;
const VALUE: u32 = 7;

/// Calls `routine(builder, <the pushed word>, line)`; `push` pushes the second argument.
fn call_with(s: &mut Asm, routine: &str, push: impl FnOnce(&mut Asm)) {
    s.push_slot(BUILDER);
    push(s);
    s.push_arg(3);
    s.call(routine);
}

/// Runs `each` for every index of slot `index` below the count in slot `count`.
pub(super) fn count_up(s: &mut Asm, index: u32, count: u32, each: impl FnOnce(&mut Asm)) {
    let (again, done) = (s.label(), s.label());
    s.i(Const(A, 0));
    s.i(StoreSlot(index, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, index));
    s.i(LoadSlot(B, count));
    s.jump_if(Ge, done);
    each(s);
    s.i(LoadSlot(A, index));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(index, A));
    s.i(Jump(again));
    s.i(Mark(done));
}

/// Puts a comma before every field but the first.
fn comma_between(s: &mut Asm) {
    let first = s.label();
    s.i(LoadSlot(A, FIELD));
    s.i(Const(B, 0));
    s.jump_if(Eq, first);
    call_with(s, "csv_byte", |s| s.push_const(i64::from(b',')));
    s.i(Mark(first));
}

/// The field descriptor of the field in slot `FIELD`, into `a`.
fn field_desc(s: &mut Asm) {
    s.i(LoadSlot(A, FIELD));
    s.i(Shift(Shift::Left, 4));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    load(s, A, A, 32);
}

/// Appends the value of the field in slot `FIELD` of the record in slot `RECORD`.
fn put_cell(s: &mut Asm) {
    let (text, truth, no, done) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadSlot(A, FIELD));
    s.i(Shift(Shift::Left, 3));
    s.i(LoadSlot(B, RECORD));
    s.i(Int(IntOp::Add));
    load(s, A, A, 0);
    s.i(StoreSlot(VALUE, A));
    field_desc(s);
    load(s, A, A, 0);
    s.i(Const(B, 1));
    s.jump_if(Eq, text);
    s.i(Const(B, 2));
    s.jump_if(Eq, truth);
    s.push_slot(VALUE);
    s.call("number_format");
    s.i(Move { to: B, from: A });
    call_with(s, "csv_add", |s| s.i(Push(B)));
    s.i(Jump(done));
    s.i(Mark(text));
    // An empty text in a one-field table is `""`: a bare empty line would read as no row.
    let cell = s.label();
    s.i(LoadSlot(A, VALUE));
    load(s, A, A, 0);
    s.i(Const(B, 0));
    s.jump_if(Ne, cell);
    s.i(LoadSlot(A, FIELDS));
    s.i(Const(B, 1));
    s.jump_if(Ne, cell);
    for _ in 0..2 {
        call_with(s, "csv_byte", |s| s.push_const(i64::from(b'"')));
    }
    s.i(Jump(done));
    s.i(Mark(cell));
    call_with(s, "csv_cell", |s| s.push_slot(VALUE));
    s.i(Jump(done));
    s.i(Mark(truth));
    s.i(LoadSlot(A, VALUE));
    s.i(Const(B, 0));
    s.jump_if(Eq, no);
    call_with(s, "csv_add", |s| s.push_text(super::YES));
    s.i(Jump(done));
    s.i(Mark(no));
    call_with(s, "csv_add", |s| s.push_text(super::NO));
    s.i(Mark(done));
}

/// `csv_text(list, desc, line)`: a fresh text holding the header, the field names of the
/// record descriptor `desc` in order, then one line per record of `list`, each ending `\n`.
/// Numbers are in their `show` form, truths `yes` or `no`, and a text is quoted only when it
/// must be (`csv_cell`).
fn csv_text() -> Function {
    let mut s = Asm::default();
    s.push_const(24);
    s.push_arg(3);
    s.call("alloc");
    s.i(StoreSlot(BUILDER, A));
    s.i(Const(A, 0));
    s.i(Push(A));
    s.i(Push(A));
    s.push_const(32);
    s.push_arg(3);
    s.call("grow_text");
    s.i(LoadSlot(B, BUILDER));
    store(&mut s, B, 0, A);
    s.i(Const(A, 64));
    store(&mut s, B, 16, A);
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 16);
    s.i(StoreSlot(FIELDS, A));
    count_up(&mut s, FIELD, FIELDS, |s| {
        comma_between(s);
        s.i(LoadSlot(A, FIELD));
        s.i(Shift(Shift::Left, 4));
        s.i(LoadArg(B, 2));
        s.i(Int(IntOp::Add));
        load(s, A, A, 24);
        s.i(Move { to: B, from: A });
        call_with(s, "csv_add", |s| s.i(Push(B)));
    });
    call_with(&mut s, "csv_byte", |s| s.push_const(i64::from(b'\n')));
    s.i(LoadArg(A, 1));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(ROWS, A));
    count_up(&mut s, ROW, ROWS, |s| {
        s.i(LoadArg(A, 1));
        load(s, B, A, 16);
        s.i(LoadSlot(A, ROW));
        s.i(Shift(Shift::Left, 3));
        s.i(Int(IntOp::Add));
        load(s, A, A, 0);
        s.i(StoreSlot(RECORD, A));
        count_up(s, FIELD, FIELDS, |s| {
            comma_between(s);
            put_cell(s);
        });
        call_with(s, "csv_byte", |s| s.push_const(i64::from(b'\n')));
    });
    s.i(LoadSlot(A, BUILDER));
    load(&mut s, B, A, 8);
    load(&mut s, A, A, 0);
    store(&mut s, A, 0, B);
    s.i(Return);
    s.finish("csv_text", 3, 7)
}

pub fn routines() -> Vec<Function> {
    vec![csv_byte(), csv_add(), csv_cell(), csv_text()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Reg::{A, B};
    use crate::ir::{Data, Inst, Inst::*};

    /// What a builder with room for 64 bytes holds after the instructions `calls`, which
    /// find the builder in slot 1, have run.
    fn built(calls: Vec<Inst>, texts: &[&[u8]]) -> Vec<u8> {
        let mut body = vec![Const(A, 24), Push(A), Const(A, 0), Push(A)];
        body.extend([Call("alloc".into()), StoreSlot(1, A)]);
        body.extend([Const(A, 72), Push(A), Const(A, 0), Push(A)]);
        body.extend([Call("alloc".into()), Move { to: B, from: A }]);
        body.extend([
            LoadSlot(A, 1),
            Store {
                base: A,
                offset: 0,
                from: B,
            },
        ]);
        body.extend([
            Const(B, 64),
            Store {
                base: A,
                offset: 16,
                from: B,
            },
        ]);
        body.extend(calls);
        body.extend([Const(A, 1), Push(A), LoadSlot(A, 1)]);
        body.extend([
            Load {
                to: B,
                base: A,
                offset: 0,
            },
            AddImm(B, 8),
            Push(B),
        ]);
        body.extend([
            Load {
                to: A,
                base: A,
                offset: 8,
            },
            Push(A),
        ]);
        body.push(Call("write_all".into()));
        let mut m = program(body, texts);
        m.functions[0].slots = 1;
        m.functions[0].body[0] = Enter(1);
        let out = run(&m);
        assert_eq!(out.status, 0);
        out.stdout
    }

    /// Runs `routine(builder, text 0, 0)` on a fresh builder.
    fn with_text(routine: &str, text: &[u8]) -> Vec<u8> {
        let mut call = vec![
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Text(0)),
            Push(A),
        ];
        call.extend([Const(A, 0), Push(A), Call(routine.into())]);
        built(call, &[text])
    }

    #[test]
    fn bytes_are_appended_in_order() {
        let bytes = b"a,\"b\n";
        let mut calls = Vec::new();
        for byte in bytes {
            calls.extend([LoadSlot(A, 1), Push(A), Const(A, i64::from(*byte)), Push(A)]);
            calls.extend([Const(A, 0), Push(A), Call("csv_byte".into())]);
        }
        assert_eq!(built(calls, &[]), bytes);
        assert_eq!(built(vec![], &[]), b"");
    }

    #[test]
    fn a_builder_that_is_full_grows_and_keeps_every_byte() {
        let long: Vec<u8> = (0..1000).map(|i| b'a' + (i % 26) as u8).collect();
        assert_eq!(with_text("csv_add", &long), long);
    }

    #[test]
    fn a_cell_is_quoted_only_for_a_comma_a_quote_or_a_line_break() {
        let cell = |text: &[u8]| with_text("csv_cell", text);
        assert_eq!(cell(b"a b"), b"a b");
        assert_eq!(cell(b""), b"");
        assert_eq!(cell(b"x,y"), b"\"x,y\"");
        assert_eq!(cell(b"say \"hi\""), b"\"say \"\"hi\"\"\"");
        assert_eq!(cell(b"two\nlines"), b"\"two\nlines\"");
        assert_eq!(cell(b"cr\rhere"), b"\"cr\rhere\"");
        let long = [vec![b'z'; 300], b",".to_vec()].concat();
        assert_eq!(cell(&long), [b"\"", &long[..], b"\""].concat());
    }
}

#[cfg(test)]
mod table_tests {
    use super::super::tests::{program, run};
    use crate::ir::{
        Data, Desc,
        Inst::*,
        Module,
        Reg::{A, B},
    };

    /// A row of the table `E` with the fields `name` (text), `amount` (number), `paid` (truth).
    type Row = (u32, i64, i64);

    /// The CSV text of the table holding `rows`; `texts` are the names' text, by index.
    fn csv(texts: &[&[u8]], rows: &[Row]) -> Vec<u8> {
        csv_of(texts, rows, 3)
    }

    /// Like `csv`, for a table with only its first `n` fields.
    fn csv_of(texts: &[&[u8]], rows: &[Row], n: usize) -> Vec<u8> {
        let mut descs = Module::default();
        let kinds = [Desc::Text, Desc::Number, Desc::Truth];
        let fields = ["name", "amount", "paid"];
        let list = (0..n)
            .map(|i| (fields[i].to_string(), descs.desc(kinds[i].clone())))
            .collect();
        let at = descs.desc(Desc::Record {
            name: "E".into(),
            fields: list,
        });
        let mut body = vec![Const(A, 4), Push(A), Const(A, 0), Push(A)];
        body.extend([Call("list_new".into()), StoreSlot(1, A)]);
        for (name, amount, paid) in rows {
            body.extend([Const(A, 24), Push(A), Const(A, 0), Push(A)]);
            body.extend([Call("alloc".into()), StoreSlot(2, A)]);
            let cells = [
                AddressOf(B, Data::Text(*name)),
                Const(B, *amount),
                Const(B, *paid),
            ];
            for (i, cell) in cells.into_iter().take(n).enumerate() {
                let store = Store {
                    base: A,
                    offset: 8 * i as i32,
                    from: B,
                };
                body.extend([cell, store]);
            }
            body.extend([LoadSlot(A, 1), Push(A), LoadSlot(A, 2), Push(A)]);
            body.extend([Const(A, 0), Push(A), Call("list_push".into())]);
        }
        body.extend([
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Desc(at)),
            Push(A),
        ]);
        body.extend([Const(A, 0), Push(A), Call("csv_text".into())]);
        body.extend([Push(A), Call("out_text".into()), Call("flush".into())]);
        let mut m = program(body, texts);
        m.functions[0].slots = 2;
        m.functions[0].body[0] = Enter(2);
        m.descs = descs.descs;
        let out = run(&m);
        assert_eq!(out.status, 0);
        out.stdout
    }

    #[test]
    fn an_empty_table_is_its_header_line() {
        assert_eq!(csv(&[], &[]), b"name,amount,paid\n");
    }

    #[test]
    fn each_record_is_a_line_of_show_numbers_yes_or_no_and_quoted_text() {
        let rows = [(0, 1_205_000, 1), (1, -30_000, 0), (0, 0, 1), (2, 5, 0)];
        let texts: [&[u8]; 3] = [b"tea", b"a, \"b\"", b""];
        let want =
            "name,amount,paid\ntea,120.5,yes\n\"a, \"\"b\"\"\",-3,no\ntea,0,yes\n,0.0005,no\n";
        assert_eq!(csv(&texts, &rows), want.as_bytes());
    }

    #[test]
    fn a_table_longer_than_the_first_room_is_written_in_full() {
        let long = vec![b'z'; 200];
        let mut want = b"name,amount,paid\n".to_vec();
        for _ in 0..30 {
            want.extend([&long[..], b",1,yes\n"].concat());
        }
        assert_eq!(csv(&[&long], &[(0, 10_000, 1); 30]), want);
    }

    #[test]
    fn text_is_written_byte_for_byte_and_quoted_only_when_it_must_be() {
        let texts: [&[u8]; 6] = [
            "café".as_bytes(),
            "日本".as_bytes(),
            b"  padded  ",
            b"\"",
            b"a\rb",
            b"x",
        ];
        let rows: Vec<Row> = (0..6).map(|i| (i, 0, 0)).collect();
        let want = "name,amount,paid\ncafé,0,no\n日本,0,no\n  padded  ,0,no\n\"\"\"\"\
                    ,0,no\n\"a\rb\",0,no\nx,0,no\n";
        assert_eq!(csv(&texts, &rows), want.as_bytes());
    }

    #[test]
    fn an_empty_text_is_quoted_when_it_is_the_only_field_so_the_row_survives() {
        let texts: [&[u8]; 2] = [b"", b"tea"];
        let rows: [Row; 3] = [(1, 0, 0), (0, 0, 0), (0, 0, 0)];
        assert_eq!(csv_of(&texts, &rows, 1), b"name\ntea\n\"\"\n\"\"\n");
        // With more than one field an empty text stays bare.
        assert_eq!(csv(&texts, &[(0, 0, 0)]), b"name,amount,paid\n,0,no\n");
    }
}
