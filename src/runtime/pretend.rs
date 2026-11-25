//! Pretend files: in a test build each table has an in-memory file instead of a real one.
//! The file is the address of a CSV text in the table's entry of the tables block, which
//! holds `[list, loaded flag, pretend file]` (24 bytes) per table in a test build. 0 is a
//! file that is missing. The CSV is parsed by `table_rows` and written by `csv_text`, the
//! code a real file goes through, so a test sees what a real `load` would.

use super::asm::Asm;
use crate::ir::Cond::Eq;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Global, Inst::*, IntOp};

/// The bytes of a table's entry in a test build.
pub const ENTRY_BYTES: i32 = 24;
/// Where the pretend file is in an entry.
pub const FILE_AT: i32 = 16;

/// `a` = the address of the pretend file word of the table whose index is argument 1.
fn file_word(s: &mut Asm) {
    s.i(LoadArg(A, 1));
    s.i(Const(B, i64::from(ENTRY_BYTES)));
    s.i(Int(IntOp::Mul));
    s.i(StoreSlot(2, A));
    s.gload(A, Global::Tables);
    s.i(LoadSlot(B, 2));
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, FILE_AT));
}

/// `pretend_save(index, list, desc, line)`: the table `list` of records of `desc` becomes the
/// pretend file of table `index`, as the text `csv_text` makes of it. An earlier file is
/// replaced. Slots: 1 the text, 2 the entry's offset.
fn pretend_save() -> Function {
    let mut s = Asm::default();
    for arg in [2, 3, 4] {
        s.push_arg(arg);
    }
    s.call("csv_text");
    s.i(StoreSlot(1, A));
    file_word(&mut s);
    s.i(LoadSlot(B, 1));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.finish("pretend_save", 4, 2)
}

/// `pretend_read(index, shown, desc, line)`: the rows of the pretend file of table `index`
/// as a new list of records of `desc`; a missing file is an empty list, with no notice.
/// Slots: 1 the list, 2 the entry's offset.
fn pretend_read() -> Function {
    let mut s = Asm::default();
    let missing = s.label();
    s.push_const(4);
    s.push_arg(4);
    s.call("list_new");
    s.i(StoreSlot(1, A));
    file_word(&mut s);
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 0));
    s.jump_if(Eq, missing);
    s.i(Push(A));
    for arg in [2, 3, 4] {
        s.push_arg(arg);
    }
    s.push_slot(1);
    s.call("table_rows");
    s.i(Return);
    s.i(Mark(missing));
    s.i(LoadSlot(A, 1));
    s.i(Return);
    s.finish("pretend_read", 4, 2)
}

pub fn routines() -> Vec<Function> {
    vec![pretend_read(), pretend_save()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::{
        Data, Desc, Global,
        Inst::*,
        Module,
        Reg::{A, B},
    };

    /// Reads table `index` of a two-table block, 24 bytes an entry, in which table `at`'s
    /// pretend file (at +16) is the text `file` (if any), as `Expense` records, and shows the list. Gives the status,
    /// stdout and stderr.
    fn read(file: Option<(i64, &str)>, index: i64) -> (i32, String, String) {
        let mut descs = Module::default();
        let kinds = [Desc::Text, Desc::Number, Desc::Truth];
        let names = ["name", "amount", "paid"].map(String::from);
        let fields = names
            .into_iter()
            .zip(kinds.map(|k| descs.desc(k)))
            .collect();
        let record = descs.desc(Desc::Record {
            name: "Expense".into(),
            fields,
        });
        let list = descs.desc(Desc::List(record));
        let size = 48;
        let mut body = vec![Const(A, size), Push(A), Const(A, 0), Push(A)];
        body.extend([Call("alloc".into()), StoreSlot(1, A)]);
        body.extend([AddressOf(B, Data::Global(Global::Tables))]);
        body.extend([Store {
            base: B,
            offset: 0,
            from: A,
        }]);
        if let Some((at, _)) = file {
            body.extend([
                AddressOf(B, Data::Text(1)),
                LoadSlot(A, 1),
                AddImm(A, 24 * at as i32),
            ]);
            body.extend([Store {
                base: A,
                offset: 16,
                from: B,
            }]);
        }
        body.extend([
            Const(A, index),
            Push(A),
            AddressOf(A, Data::Text(0)),
            Push(A),
        ]);
        body.extend([
            AddressOf(A, Data::Desc(record)),
            Push(A),
            Const(A, 7),
            Push(A),
        ]);
        body.extend([Call("pretend_read".into()), Push(A)]);
        body.extend([AddressOf(A, Data::Desc(list)), Push(A)]);
        body.extend([Call("show_value".into()), Call("flush".into())]);
        let texts: [&[u8]; 2] = [b"e.csv", file.map_or("", |f| f.1).as_bytes()];
        let mut m = program(body, &texts);
        m.functions[0].slots = 1;
        m.functions[0].body[0] = Enter(1);
        m.descs = descs.descs;
        let o = run(&m);
        let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
        (o.status, text(o.stdout), text(o.stderr))
    }

    #[test]
    fn a_pretend_file_is_read_like_a_real_one() {
        let file = "paid,name,amount\nyes,\"A, b\",1\nno,C,-2.5\n";
        let want = "list of (Expense with name \"A, b\", amount 1, paid yes), \
                    (Expense with name \"C\", amount -2.5, paid no)";
        assert_eq!(read(Some((0, file)), 0), (0, want.into(), String::new()));
    }

    #[test]
    fn a_missing_pretend_file_loads_empty_and_prints_no_notice() {
        let want = (0, "empty list of Expense".to_string(), String::new());
        assert_eq!(read(None, 0), want);
    }

    #[test]
    fn a_second_table_has_its_own_file() {
        let file = "name,amount,paid\nA,1,yes\n";
        let want = "list of (Expense with name \"A\", amount 1, paid yes)";
        let empty = "empty list of Expense";
        for (at, index, shown) in [(1, 1, want), (1, 0, empty), (0, 1, empty), (0, 0, want)] {
            let got = read(Some((at, file)), index);
            assert_eq!(
                got,
                (0, shown.into(), String::new()),
                "file {at} read {index}"
            );
        }
    }

    #[test]
    fn pretend_load_uses_the_real_parser() {
        let (status, shown, err) = read(Some((0, "name,amount,paid\nA,x,yes\n")), 0);
        let msg = "line 7: failure 6: e.csv line 2: amount \"x\" is not a number\n";
        assert_eq!((status, shown.as_str(), err.as_str()), (2, "", msg));
    }
}

#[cfg(test)]
mod save_tests {
    use super::super::tests::{program, run};
    use crate::ir::{
        Data, Desc, Global,
        Inst::*,
        Module,
        Reg::{A, B},
    };

    /// A row of `Expense`: the index of its name among the texts after `NAMES`, a number, a truth.
    type Row = (u32, i64, i64);
    /// What a session does with a two-table block (24 bytes an entry) of `Expense` tables.
    enum Step {
        Save(i64, Vec<Row>),
        Read(i64),
        /// Writes the text of the pretend file word of the table.
        Word(i64),
    }
    /// Texts 0 and 1 are the shown name `e.csv` and the separator written after each step.
    const NAMES: [&[u8]; 2] = [b"tea", b"a, \"b\""];

    fn session(steps: &[Step]) -> (i32, String, String) {
        let mut descs = Module::default();
        let kinds = [Desc::Text, Desc::Number, Desc::Truth];
        let fields = ["name", "amount", "paid"].map(String::from);
        let fields = fields.into_iter().zip(kinds.map(|k| descs.desc(k)));
        let record = descs.desc(Desc::Record {
            name: "Expense".into(),
            fields: fields.collect(),
        });
        let list = descs.desc(Desc::List(record));
        let push = |a| [AddressOf(A, a), Push(A)];
        let mut body = vec![Const(A, 48), Push(A), Const(A, 0), Push(A)];
        body.extend([Call("alloc".into()), StoreSlot(1, A)]);
        body.extend([AddressOf(B, Data::Global(Global::Tables))]);
        body.extend([Store {
            base: B,
            offset: 0,
            from: A,
        }]);
        for step in steps {
            match step {
                Step::Save(at, rows) => {
                    body.extend([Const(A, 4), Push(A), Const(A, 0), Push(A)]);
                    body.extend([Call("list_new".into()), StoreSlot(2, A)]);
                    for (name, amount, paid) in rows {
                        body.extend([Const(A, 24), Push(A), Const(A, 0), Push(A)]);
                        body.extend([Call("alloc".into()), StoreSlot(3, A)]);
                        let cells = [
                            AddressOf(B, Data::Text(2 + name)),
                            Const(B, *amount),
                            Const(B, *paid),
                        ];
                        for (i, cell) in cells.into_iter().enumerate() {
                            body.extend([
                                cell,
                                Store {
                                    base: A,
                                    offset: 8 * i as i32,
                                    from: B,
                                },
                            ]);
                        }
                        body.extend([LoadSlot(A, 2), Push(A), LoadSlot(A, 3), Push(A)]);
                        body.extend([Const(A, 0), Push(A), Call("list_push".into())]);
                    }
                    body.extend([Const(A, *at), Push(A), LoadSlot(A, 2), Push(A)]);
                    body.extend(push(Data::Desc(record)));
                    body.extend([Const(A, 7), Push(A), Call("pretend_save".into())]);
                }
                Step::Read(at) => {
                    body.extend([Const(A, *at), Push(A)]);
                    body.extend(push(Data::Text(0)));
                    body.extend(push(Data::Desc(record)));
                    body.extend([Const(A, 7), Push(A), Call("pretend_read".into()), Push(A)]);
                    body.extend(push(Data::Desc(list)));
                    body.push(Call("show_value".into()));
                }
                Step::Word(at) => {
                    body.extend([LoadSlot(A, 1), AddImm(A, 24 * *at as i32 + 16)]);
                    body.extend([
                        Load {
                            to: A,
                            base: A,
                            offset: 0,
                        },
                        Push(A),
                        Call("out_text".into()),
                    ]);
                }
            }
            body.extend(push(Data::Text(1)));
            body.extend([Call("out_text".into())]);
        }
        body.push(Call("flush".into()));
        let mut texts: Vec<&[u8]> = vec![b"e.csv", b"|"];
        texts.extend(NAMES);
        let mut m = program(body, &texts);
        m.functions[0].slots = 3;
        m.functions[0].body[0] = Enter(3);
        m.descs = descs.descs;
        let o = run(&m);
        let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
        (o.status, text(o.stdout), text(o.stderr))
    }

    fn shown(steps: &[Step]) -> String {
        let (status, out, err) = session(steps);
        assert_eq!((status, err.as_str()), (0, ""), "{out}");
        out
    }

    const TWO: [Row; 2] = [(0, 1_205_000, 1), (1, -30_000, 0)];
    const ROWS: &str = "list of (Expense with name \"tea\", amount 120.5, paid yes), \
                        (Expense with name \"a, \"\"b\"\"\", amount -3, paid no)|";

    #[test]
    fn a_saved_table_reads_back() {
        let steps = [Step::Save(0, TWO.to_vec()), Step::Read(0)];
        assert_eq!(shown(&steps), format!("|{ROWS}"));
    }

    #[test]
    fn save_twice_keeps_the_last() {
        let steps = [
            Step::Save(1, TWO.to_vec()),
            Step::Save(1, TWO[..1].to_vec()),
            Step::Read(1),
        ];
        let one = "list of (Expense with name \"tea\", amount 120.5, paid yes)|";
        assert_eq!(shown(&steps), format!("||{one}"));
    }

    #[test]
    fn a_saved_table_does_not_fill_the_other_ones_file() {
        let steps = [Step::Save(1, TWO.to_vec()), Step::Read(0), Step::Read(1)];
        assert_eq!(shown(&steps), format!("|empty list of Expense|{ROWS}"));
    }

    #[test]
    fn pretend_save_uses_the_real_writer() {
        let want = "name,amount,paid\ntea,120.5,yes\n\"a, \"\"b\"\"\",-3,no\n|";
        let steps = [Step::Save(0, TWO.to_vec()), Step::Word(0)];
        assert_eq!(shown(&steps), format!("|{want}"));
    }
}
