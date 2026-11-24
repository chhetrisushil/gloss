//! Reading CSV text (RFC 4180): a cursor over the text that gives one row at a time.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Label};

const CURSOR_BYTES: i64 = 32;
// Words of a cursor: the text, the position in it (at 8), the line the next row starts on, and
// the line the row being read started on.
const POS: i32 = 8;
const NEXT_LINE: i32 = 16;
const ROW_LINE: i32 = 24;

/// `csv_cursor(text, line)`: a cursor at the start of `text`, whose first row is line 1.
fn csv_cursor() -> Function {
    let mut s = Asm::default();
    s.push_const(CURSOR_BYTES);
    s.push_arg(2);
    s.call("alloc");
    s.i(LoadArg(B, 1));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.i(Const(B, 1));
    for offset in [NEXT_LINE, ROW_LINE] {
        s.i(Store {
            base: A,
            offset,
            from: B,
        });
    }
    s.i(Return);
    s.finish("csv_cursor", 2, 0)
}

/// `fail_row(line, shown, row)`: starts failure 6 for the row of the table file `shown`
/// that begins on file line `row`: `<shown> line <row>: `. The caller says what is wrong
/// and calls `fail_end`.
fn fail_row() -> Function {
    let mut s = Asm::default();
    s.push_arg(1);
    s.push_const(6);
    s.call("fail_begin");
    s.push_arg(2);
    s.call("err_text");
    s.push_text(super::LINE_N);
    s.call("err_text");
    s.push_arg(3);
    s.call("err_int");
    s.push_text(super::COLON);
    s.call("err_text");
    s.finish("fail_row", 3, 0)
}

// Slots of `csv_read_cell`.
const BUILDER: u32 = 1;
const TEXT: u32 = 2;
const AT: u32 = 3;
const END: u32 = 4;
const LINE: u32 = 5;

/// The byte of the text `delta` bytes after the position, into `a`.
fn byte_at(s: &mut Asm, delta: i32) {
    s.i(LoadSlot(A, TEXT));
    s.i(LoadSlot(B, AT));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8 + delta,
    });
}

/// Jumps to `to` when the position, plus `delta`, is at or past the end of the text.
fn at_end(s: &mut Asm, delta: i32, to: Label) {
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, delta));
    s.i(LoadSlot(B, END));
    s.jump_if(Ge, to);
}

/// Appends the byte in `a` to the cell and moves on by `step` bytes.
fn take(s: &mut Asm, step: i32) {
    s.i(Move { to: B, from: A });
    s.push_slot(BUILDER);
    s.i(Push(B));
    s.push_arg(3);
    s.call("csv_byte");
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, step));
    s.i(StoreSlot(AT, A));
}

/// `csv_read_cell(cursor, shown, line)`: reads the cell at the cursor and leaves the cursor on the
/// `,`, the line break or the end of the text that follows it. A cell is taken as it is, up
/// to `,`, `\n`, `\r\n` or the end of the text: a lone `\r` and a `"` belong to it, and
/// nothing is trimmed. Slots: the builder (the text so far, its length and its room), the
/// text, the position, its length, the next row's line.
fn csv_read_cell() -> Function {
    let mut s = Asm::default();
    let [plain, tail, done] = [(); 3].map(|_| s.label());
    s.push_const(24);
    s.push_arg(3);
    s.call("alloc");
    s.i(StoreSlot(BUILDER, A));
    s.i(Const(A, 0));
    s.i(Push(A));
    s.i(Push(A));
    s.push_const(8);
    s.push_arg(3);
    s.call("grow_text");
    s.i(LoadSlot(B, BUILDER));
    s.i(Store {
        base: B,
        offset: 0,
        from: A,
    });
    s.i(Const(A, 16));
    s.i(Store {
        base: B,
        offset: 16,
        from: A,
    });
    for (slot, offset) in [(TEXT, 0), (AT, POS), (LINE, NEXT_LINE)] {
        s.i(LoadArg(A, 1));
        s.i(Load {
            to: A,
            base: A,
            offset,
        });
        s.i(StoreSlot(slot, A));
    }
    s.i(LoadSlot(A, TEXT));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(END, A));
    s.i(Mark(tail));
    at_end(&mut s, 0, done);
    byte_at(&mut s, 0);
    for stop in *b",\n" {
        s.i(Const(B, i64::from(stop)));
        s.jump_if(Eq, done);
    }
    s.i(Const(B, i64::from(b'\r')));
    s.jump_if(Ne, plain);
    at_end(&mut s, 1, plain);
    byte_at(&mut s, 1);
    s.i(Const(B, i64::from(b'\n')));
    s.jump_if(Eq, done);
    s.i(Mark(plain));
    byte_at(&mut s, 0);
    take(&mut s, 1);
    s.i(Jump(tail));
    s.i(Mark(done));
    s.i(LoadArg(B, 1));
    s.i(LoadSlot(A, AT));
    s.i(Store {
        base: B,
        offset: POS,
        from: A,
    });
    s.i(LoadSlot(A, LINE));
    s.i(Store {
        base: B,
        offset: NEXT_LINE,
        from: A,
    });
    s.i(LoadSlot(B, BUILDER));
    s.i(Load {
        to: A,
        base: B,
        offset: 8,
    });
    s.i(Load {
        to: B,
        base: B,
        offset: 0,
    });
    s.i(Store {
        base: B,
        offset: 0,
        from: A,
    });
    s.i(Move { to: A, from: B });
    s.i(Return);
    s.finish("csv_read_cell", 3, 5)
}

pub fn routines() -> Vec<Function> {
    vec![csv_cursor(), fail_row(), csv_read_cell()]
}

#[cfg(test)]
mod tests {
    use super::super::add_reachable;
    use super::super::tests::{program, run};
    use crate::ir::Inst::*;
    use crate::ir::{Data, Function, Inst, Module, Reg::A};
    use crate::irsim::Outcome;

    /// Runs `after` with a cursor over `input` in slot 1 of a function `t`; `shown` is text 1
    /// (`e.csv`), a line feed is text 2 and a space text 3.
    fn with_cursor(input: &[u8], after: Vec<Inst>) -> Outcome {
        let mut m: Module = program(vec![Call("t".into())], &[]);
        let [text, _shown, _lf, _space] = [input, b"e.csv", b"\n", b" "].map(|t| m.text(t));
        let mut body = vec![Enter(1), AddressOf(A, Data::Text(text)), Push(A)];
        body.extend([
            Const(A, 7),
            Push(A),
            Call("csv_cursor".into()),
            StoreSlot(1, A),
        ]);
        body.extend(after);
        body.extend([Const(A, 0), Return]);
        let called: Vec<String> = body
            .iter()
            .filter_map(|i| match i {
                Call(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        m.functions.push(Function {
            name: "t".into(),
            params: 0,
            slots: 1,
            body,
        });
        add_reachable(&mut m, &called);
        run(&m)
    }

    /// Prints, on standard error, the number in `a`, then a space.
    fn say() -> Vec<Inst> {
        let space = AddressOf(A, Data::Text(3));
        vec![
            Push(A),
            Call("err_int".into()),
            space,
            Push(A),
            Call("err_text".into()),
        ]
    }

    #[test]
    fn a_new_cursor_is_at_the_start_of_the_text_and_line_1() {
        // The length of the text it holds, then the other three words.
        let mut after = vec![
            LoadSlot(A, 1),
            Load {
                to: A,
                base: A,
                offset: 0,
            },
        ];
        after.extend([Load {
            to: A,
            base: A,
            offset: 0,
        }]);
        after.extend(say());
        for offset in [8, 16, 24] {
            after.extend([
                LoadSlot(A, 1),
                Load {
                    to: A,
                    base: A,
                    offset,
                },
            ]);
            after.extend(say());
        }
        let o = with_cursor(b"a,b\n", after);
        assert_eq!(
            (o.status, String::from_utf8(o.stderr).unwrap()),
            (0, "4 0 1 1 ".into())
        );
    }

    #[test]
    fn fail_row_names_the_file_and_the_line_the_row_started_on() {
        let mut after = vec![Const(A, 7), Push(A), AddressOf(A, Data::Text(1)), Push(A)];
        after.extend([
            Const(A, 9),
            Push(A),
            Call("fail_row".into()),
            Call("fail_end".into()),
        ]);
        let o = with_cursor(b"", after);
        assert_eq!(o.status, 2);
        assert_eq!(
            String::from_utf8(o.stderr).unwrap(),
            "line 7: failure 6: e.csv line 9: \n"
        );
    }

    /// Reads one cell of `input` (the failure messages call the file `e.csv`, and the row
    /// started on line 9): the cell, then where the cursor is and the line it is on.
    fn cell(input: &[u8]) -> (Outcome, String) {
        let mut after = vec![
            LoadSlot(A, 1),
            Push(A),
            AddressOf(A, Data::Text(1)),
            Push(A),
        ];
        after.extend([Const(A, 7), Push(A), Call("csv_read_cell".into())]);
        after.extend([Push(A), Call("out_quoted".into()), Call("flush".into())]);
        for offset in [8, 16] {
            after.extend([
                LoadSlot(A, 1),
                Load {
                    to: A,
                    base: A,
                    offset,
                },
            ]);
            after.extend(say());
        }
        let o = with_cursor(input, after);
        let seen = String::from_utf8(o.stderr.clone()).unwrap();
        (o, seen)
    }

    #[test]
    fn a_plain_cell_runs_to_a_comma_a_line_break_or_the_end_and_is_never_trimmed() {
        // The input, the cell, and the position the cursor is left at.
        let cases: [(&[u8], &str, usize); 12] = [
            (b"abc,d", "abc", 3),
            (b"", "", 0),
            (b",x", "", 0),
            (b"a\nb", "a", 1),
            (b"a\r\nb", "a", 1),
            (b"a\rb", "a\rb", 3),
            (b"a\r", "a\r", 2),
            (b"  a b ,c", "  a b ", 6),
            (b"ab\"c,d", "ab\"c", 4),
            ("h\u{e9}llo,\u{1F600}".as_bytes(), "h\u{e9}llo", 6),
            (b"\xE2\x82\xAC", "\u{20ac}", 3),
            (b"x\r\r\n", "x\r", 2),
        ];
        for (input, want, at) in cases {
            let (o, seen) = cell(input);
            let quoted = format!("\"{}\"", want.replace('"', "\"\""));
            assert_eq!(
                String::from_utf8(o.stdout).unwrap(),
                quoted,
                "for {input:?}"
            );
            assert_eq!(seen, format!("{at} 1 "), "for {input:?}");
        }
    }
}
