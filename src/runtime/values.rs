//! Showing a value by its type descriptor (see `ir::Desc`), on standard output.

use super::asm::Asm;
use super::{CLOSE, COMMA_SPACE, LIST_OF, LIST_TRAP, NO, OPEN, QUOTE, SPACE, WITH, YES};
use crate::ir::Cond::{Eq, Ge, Le, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

/// Writes the runtime text `n` to standard output.
fn out_const(s: &mut Asm, n: u32) {
    s.push_text(n);
    s.call("out_text");
}

/// Writes the bytes of text argument 1 from the run start (slot 3) up to the index (slot 1),
/// when there are any.
fn put_run(s: &mut Asm) {
    let empty = s.label();
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 3));
    s.jump_if(Le, empty);
    s.i(LoadArg(A, 1));
    s.i(AddImm(A, 8));
    s.i(LoadSlot(B, 3));
    s.i(Int(IntOp::Add));
    s.i(Push(A));
    s.i(LoadSlot(A, 1));
    s.i(Int(IntOp::Sub));
    s.i(Push(A));
    s.call("out_bytes");
    s.i(Mark(empty));
}

/// `out_quoted(text)`: the text in double quotes, each `"` inside it doubled (as in source).
/// Each run of bytes up to and including a quote goes out in one `out_bytes` call, then the
/// doubling quote. Slots: 1 the index, 2 the length, 3 where the pending run starts.
fn out_quoted() -> Function {
    let mut s = Asm::default();
    let (again, done) = (s.label(), s.label());
    out_const(&mut s, QUOTE);
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(2, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(StoreSlot(3, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(LoadSlot(B, 1));
    s.i(AddImm(B, 1));
    s.i(StoreSlot(1, B));
    s.i(Const(B, i64::from(b'"')));
    s.jump_if(Ne, again);
    // A quote: write the run with it, write it again, start a new run after it.
    put_run(&mut s);
    out_const(&mut s, QUOTE);
    s.i(LoadSlot(A, 1));
    s.i(StoreSlot(3, A));
    s.i(Jump(again));
    s.i(Mark(done));
    put_run(&mut s);
    out_const(&mut s, QUOTE);
    s.finish("out_quoted", 1, 3)
}

/// `show_in_record(value, desc)`: a number, text or truth as a record shows its field: the
/// source form, so a text is in quotes (a top-level `show` of a text writes none).
fn show_in_record() -> Function {
    let mut s = Asm::default();
    let (number, text, truth, no) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 2));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 1));
    s.jump_if(Eq, text);
    s.i(Const(B, 2));
    s.jump_if(Eq, truth);
    s.i(Mark(number));
    s.push_arg(1);
    s.call("out_number");
    s.i(Return);
    s.i(Mark(text));
    s.push_arg(1);
    s.call("out_quoted");
    s.i(Return);
    s.i(Mark(truth));
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Eq, no);
    out_const(&mut s, YES);
    s.i(Return);
    s.i(Mark(no));
    out_const(&mut s, NO);
    s.finish("show_in_record", 2, 0)
}

/// `show_fields(value, desc)`: a record as `E with f v, g w`. Slots: 1 the field index, 2 the
/// field count. Records hold only scalars, so nothing here recurses.
fn show_fields() -> Function {
    let mut s = Asm::default();
    let (again, done, first) = (s.label(), s.label(), s.label());
    s.i(LoadArg(A, 2));
    s.i(Load {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Push(A));
    s.call("out_text");
    s.i(LoadArg(A, 2));
    s.i(Load {
        to: A,
        base: A,
        offset: 16,
    });
    s.i(StoreSlot(2, A));
    s.i(Const(B, 0));
    s.jump_if(Eq, done);
    out_const(&mut s, WITH);
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    s.i(Const(B, 0));
    s.jump_if(Eq, first);
    out_const(&mut s, COMMA_SPACE);
    s.i(Mark(first));
    // The field's name sits at `desc + 24 + 16 * i`, its descriptor 8 bytes on.
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 4));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: B,
        base: A,
        offset: 24,
    });
    s.i(Push(B));
    s.call("out_text");
    out_const(&mut s, SPACE);
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 3));
    s.i(LoadArg(B, 1));
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Push(A));
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 4));
    s.i(LoadArg(B, 2));
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset: 32,
    });
    s.i(Push(A));
    s.call("show_in_record");
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("show_fields", 2, 2)
}

/// Loads the kind word of the descriptor in slot `slot` into `a`.
fn kind_of(s: &mut Asm, slot: u32) {
    s.i(LoadSlot(A, slot));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
}

/// `show_list(list, desc)`: `list of a, b`, with a record item in parentheses. Slots: 1 the index, 2 the count, 3 the item descriptor.
fn show_list() -> Function {
    let mut s = Asm::default();
    let (again, done, first, record, named, shown) = (
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
    );
    s.i(LoadArg(A, 2));
    s.i(Load {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(StoreSlot(3, A));
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(2, A));
    s.i(Const(B, 0));
    s.jump_if(Ne, named);
    // The empty form arrives in the next commit; until then it is the trap.
    s.call("flush");
    s.push_text(LIST_TRAP);
    s.call("err_text");
    s.call("fail_end");
    s.i(Mark(named));
    out_const(&mut s, LIST_OF);
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    s.i(Const(B, 0));
    s.jump_if(Eq, first);
    out_const(&mut s, COMMA_SPACE);
    s.i(Mark(first));
    // The item is at `items + 8 * i`.
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 3));
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: B,
        base: B,
        offset: 16,
    });
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Push(A));
    s.push_slot(3);
    kind_of(&mut s, 3);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.call("show_in_record");
    s.i(Jump(shown));
    s.i(Mark(record));
    out_const(&mut s, OPEN);
    s.call("show_fields");
    out_const(&mut s, CLOSE);
    s.i(Mark(shown));
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("show_list", 2, 3)
}

/// `show_value(value, desc)`: a record, a list, or a scalar.
fn show_value() -> Function {
    let mut s = Asm::default();
    let (record, list) = (s.label(), s.label());
    s.i(LoadArg(A, 2));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.i(Const(B, 4));
    s.jump_if(Eq, list);
    s.push_arg(1);
    s.push_arg(2);
    s.call("show_in_record");
    s.i(Return);
    s.i(Mark(record));
    s.push_arg(1);
    s.push_arg(2);
    s.call("show_fields");
    s.i(Return);
    s.i(Mark(list));
    s.push_arg(1);
    s.push_arg(2);
    s.call("show_list");
    s.finish("show_value", 2, 0)
}

pub(super) fn routines() -> Vec<Function> {
    vec![
        out_quoted(),
        show_in_record(),
        show_fields(),
        show_list(),
        show_value(),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Reg::{A, B};
    use crate::ir::{Data, Desc, Inst::*, Module};

    /// A field of a record under test: its name, type and value (a text is an index into
    /// the texts; a number or truth is the word itself).
    enum F {
        Number(i64),
        Text(u32),
        Truth(i64),
    }

    /// Shows a record `E` with the named fields, built in the heap, and gives the output.
    fn shown(texts: &[&[u8]], fields: &[(&str, F)]) -> String {
        let mut descs = Module::default();
        let mut list = Vec::new();
        for (name, f) in fields {
            let d = match f {
                F::Number(_) => Desc::Number,
                F::Text(_) => Desc::Text,
                F::Truth(_) => Desc::Truth,
            };
            list.push((name.to_string(), descs.desc(d)));
        }
        let record = Desc::Record {
            name: "E".into(),
            fields: list,
        };
        let at = descs.desc(record);
        let mut body = vec![Const(A, 8 * fields.len() as i64), Push(A)];
        body.extend([Const(A, 0), Push(A), Call("alloc".into()), StoreSlot(1, A)]);
        for (i, (_, f)) in fields.iter().enumerate() {
            match f {
                F::Number(n) | F::Truth(n) => body.push(Const(B, *n)),
                F::Text(t) => body.push(AddressOf(B, Data::Text(*t))),
            }
            let store = Store {
                base: A,
                offset: 8 * i as i32,
                from: B,
            };
            body.extend([LoadSlot(A, 1), store]);
        }
        body.extend([
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Desc(at)),
            Push(A),
        ]);
        body.extend([Call("show_value".into()), Call("flush".into())]);
        let mut m = program(body, texts);
        m.functions[0].slots = 1;
        m.functions[0].body[0] = Enter(1);
        m.descs = descs.descs;
        String::from_utf8(run(&m).stdout).unwrap()
    }

    #[test]
    fn a_record_shows_its_fields_in_source_form() {
        let out = shown(
            &[b"say \"hi\"", b""],
            &[
                ("name", F::Text(0)),
                ("amount", F::Number(-15000)),
                ("paid", F::Truth(0)),
                ("e", F::Text(1)),
                ("ok", F::Truth(1)),
            ],
        );
        assert_eq!(
            out,
            "E with name \"say \"\"hi\"\"\", amount -1.5, paid no, e \"\", ok yes"
        );
    }

    #[test]
    fn a_record_with_one_field_has_no_comma() {
        assert_eq!(shown(&[], &[("n", F::Number(10000))]), "E with n 1");
    }

    #[test]
    fn a_long_text_with_quotes_is_doubled_run_by_run() {
        let mut text = vec![b'x'; 5000];
        for at in [0, 1, 2, 2500, 4999] {
            text[at] = b'"';
        }
        let want: String = text
            .iter()
            .flat_map(|&b| if b == b'"' { vec![b'"'; 2] } else { vec![b] })
            .map(char::from)
            .collect();
        let out = shown(&[&text], &[("t", F::Text(0))]);
        assert_eq!(out, format!("E with t \"{want}\""));
    }

    /// Shows a list of numbers with `words` as its items, built in the heap: a header
    /// `[count, capacity, items]` with the items right after it.
    fn shown_list(words: &[i64]) -> String {
        let mut descs = Module::default();
        let item = descs.desc(Desc::Number);
        let list = descs.desc(Desc::List(item));
        let size = 24 + 8 * words.len() as i64;
        let mut body = vec![Const(A, size), Push(A), Const(A, 0), Push(A)];
        body.extend([Call("alloc".into()), StoreSlot(1, A)]);
        let stores = [(0, words.len() as i64), (8, words.len() as i64)];
        for (offset, word) in stores
            .into_iter()
            .chain((24..).step_by(8).zip(words.iter().copied()))
        {
            body.extend([Const(B, word), LoadSlot(A, 1)]);
            body.push(Store {
                base: A,
                offset,
                from: B,
            });
        }
        body.extend([LoadSlot(B, 1), AddImm(B, 24), LoadSlot(A, 1)]);
        body.push(Store {
            base: A,
            offset: 16,
            from: B,
        });
        body.extend([
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Desc(list)),
            Push(A),
        ]);
        body.extend([Call("show_value".into()), Call("flush".into())]);
        let mut m = program(body, &[]);
        m.functions[0].slots = 1;
        m.functions[0].body[0] = Enter(1);
        m.descs = descs.descs;
        String::from_utf8(run(&m).stdout).unwrap()
    }

    #[test]
    fn a_list_shows_its_items() {
        assert_eq!(shown_list(&[40000, 90000]), "list of 4, 9");
    }
}
