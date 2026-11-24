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
/// `,`, the line break or the end of the text that follows it. A cell that starts with `"`
/// runs to the closing `"`, `""` being one `"`, and may hold commas and line breaks; any
/// text after the closing quote up to the next separator is kept as it is. Any other cell is
/// taken as it is, up to `,`, `\n`, `\r\n` or the end of the text: a lone `\r` and a `"` in
/// the middle belong to it. Nothing is trimmed. A quoted cell that reaches the end of the
/// text is failure 6, on the line the row started on. Slots: the builder (the text so far,
/// its length and its room), the text, the position, its length, the next row's line.
fn csv_read_cell() -> Function {
    let mut s = Asm::default();
    let [quoted, quote, plain, tail, done, other, open] = [(); 7].map(|_| s.label());
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
    at_end(&mut s, 0, tail);
    byte_at(&mut s, 0);
    s.i(Const(B, i64::from(b'"')));
    s.jump_if(Ne, tail);
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(AT, A));
    s.i(Mark(quoted));
    at_end(&mut s, 0, open);
    byte_at(&mut s, 0);
    s.i(Const(B, i64::from(b'"')));
    s.jump_if(Eq, quote);
    s.i(Const(B, i64::from(b'\n')));
    s.jump_if(Ne, other);
    s.i(LoadSlot(A, LINE));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(LINE, A));
    s.i(Mark(other));
    byte_at(&mut s, 0);
    take(&mut s, 1);
    s.i(Jump(quoted));
    // A quote ends the cell, unless another follows it.
    s.i(Mark(quote));
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(AT, A));
    at_end(&mut s, 0, tail);
    byte_at(&mut s, 0);
    s.i(Const(B, i64::from(b'"')));
    s.jump_if(Ne, tail);
    take(&mut s, 1);
    s.i(Jump(quoted));
    s.i(Mark(open));
    s.push_arg(3);
    s.push_arg(2);
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: ROW_LINE,
    });
    s.i(Push(A));
    s.call("fail_row");
    s.push_text(super::UNCLOSED);
    s.call("err_text");
    s.call("fail_end");
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

const LIST: u32 = 1; // the slot of `csv_next`

/// Loads the cursor word at `offset` into `a`.
fn cursor_word(s: &mut Asm, offset: i32) {
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset,
    });
}

/// Adds `n` to the cursor word at `offset`.
fn bump_word(s: &mut Asm, offset: i32, n: i32) {
    s.i(LoadArg(B, 1));
    s.i(Load {
        to: A,
        base: B,
        offset,
    });
    s.i(AddImm(A, n));
    s.i(Store {
        base: B,
        offset,
        from: A,
    });
}

/// Jumps to `to` when the cursor is at or past the end of its text.
fn at_text_end(s: &mut Asm, to: Label) {
    cursor_word(s, 0);
    s.i(Load {
        to: B,
        base: A,
        offset: 0,
    });
    cursor_word(s, POS);
    s.jump_if(Ge, to);
}

/// `csv_next(cursor, shown, line)`: the next row as a list of cell texts, or 0 at the end of
/// the text, leaving the row's first line in the cursor. A comma means another cell follows,
/// even at the end of the text; a line feed, a CR LF pair or the end of the text ends the
/// row. The end of the text right after a line break is the end, not an empty row.
fn csv_next() -> Function {
    let mut s = Asm::default();
    let [more, comma, one, done, end] = [(); 5].map(|_| s.label());
    at_text_end(&mut s, end);
    cursor_word(&mut s, NEXT_LINE);
    s.i(LoadArg(B, 1));
    s.i(Store {
        base: B,
        offset: ROW_LINE,
        from: A,
    });
    s.push_const(4);
    s.push_arg(3);
    s.call("list_new");
    s.i(StoreSlot(LIST, A));
    s.i(Mark(more));
    s.push_arg(1);
    s.push_arg(2);
    s.push_arg(3);
    s.call("csv_read_cell");
    s.i(Move { to: B, from: A });
    s.push_slot(LIST);
    s.i(Push(B));
    s.push_arg(3);
    s.call("list_push");
    at_text_end(&mut s, done);
    cursor_word(&mut s, POS);
    s.i(Move { to: B, from: A });
    cursor_word(&mut s, 0);
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Const(B, i64::from(b',')));
    s.jump_if(Eq, comma);
    s.i(Const(B, i64::from(b'\r')));
    s.jump_if(Ne, one);
    bump_word(&mut s, POS, 1);
    s.i(Mark(one));
    bump_word(&mut s, POS, 1);
    bump_word(&mut s, NEXT_LINE, 1);
    s.i(Jump(done));
    s.i(Mark(comma));
    bump_word(&mut s, POS, 1);
    s.i(Jump(more));
    s.i(Mark(done));
    s.i(LoadSlot(A, LIST));
    s.i(Return);
    s.i(Mark(end));
    s.finish("csv_next", 3, 1)
}

pub fn routines() -> Vec<Function> {
    vec![csv_cursor(), fail_row(), csv_read_cell(), csv_next()]
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::add_reachable;
    use super::super::tests::{program, run};
    use crate::ir::Cond::Eq;
    use crate::ir::Inst::*;
    use crate::ir::{
        Data, Desc, Function, Inst, Label, Module,
        Reg::{A, B},
    };
    use crate::irsim::Outcome;

    /// Runs `after` with a cursor over `input` in slot 1 of a function `t` (slots 2 to 4 are free); `shown` is text 0
    /// (`e.csv`), a line feed is text 1 and a space text 2; the descriptors are text, list of
    /// text, number, truth and the record `Expense` with fields `name`, `amount` and `paid`.
    pub(in crate::runtime) fn with_cursor(input: &[u8], after: Vec<Inst>) -> Outcome {
        let mut m: Module = program(vec![Call("t".into())], &[]);
        let text_desc = m.desc(Desc::Text);
        m.desc(Desc::List(text_desc));
        let (number, truth) = (m.desc(Desc::Number), m.desc(Desc::Truth));
        let fields = ["name", "amount", "paid"].map(String::from);
        let fields = fields.into_iter().zip([text_desc, number, truth]).collect();
        m.desc(Desc::Record {
            name: "Expense".into(),
            fields,
        });
        let [_shown, _lf, _space, text] = [&b"e.csv"[..], b"\n", b" ", input].map(|t| m.text(t));
        let mut body = vec![Enter(4), AddressOf(A, Data::Text(text)), Push(A)];
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
            slots: 4,
            body,
        });
        add_reachable(&mut m, &called);
        run(&m)
    }

    /// Prints, on standard error, the number in `a`, then a space.
    fn say() -> Vec<Inst> {
        let space = AddressOf(A, Data::Text(2));
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
        let mut after = vec![Const(A, 7), Push(A), AddressOf(A, Data::Text(0)), Push(A)];
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
        let row = Store {
            base: A,
            offset: 24,
            from: B,
        };
        let mut after = vec![Const(B, 9), LoadSlot(A, 1), row, LoadSlot(A, 1), Push(A)];
        after.extend([AddressOf(A, Data::Text(0)), Push(A), Const(A, 7), Push(A)]);
        after.push(Call("csv_read_cell".into()));
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

    #[test]
    fn a_quoted_cell_holds_commas_quotes_and_line_breaks_and_counts_the_lines() {
        // The input, the cell, the position the cursor is left at, and its line.
        let cases: [(&[u8], &str, usize, u32); 11] = [
            (b"\"a,b\",c", "a,b", 5, 1),
            (b"\"a\"\"b\"", "a\"b", 6, 1),
            (b"\"\"", "", 2, 1),
            (b"\"\"\"\"", "\"", 4, 1),
            (b"\"\"\"\"\"\",x", "\"\"", 6, 1),
            (b"\"a\nb\",c", "a\nb", 5, 2),
            (b"\"a\r\nb\"\r\nz", "a\r\nb", 6, 2),
            (b"\"\n\n\n\"", "\n\n\n", 5, 4),
            (b"\"x\"y,z", "xy", 4, 1),
            (b"\"x\" ,z", "x ", 4, 1),
            ("\"\u{e9},\u{fc}\"".as_bytes(), "\u{e9},\u{fc}", 7, 1),
        ];
        for (input, want, at, line) in cases {
            let (o, seen) = cell(input);
            let quoted = format!("\"{}\"", want.replace('"', "\"\""));
            assert_eq!(
                String::from_utf8(o.stdout).unwrap(),
                quoted,
                "for {input:?}"
            );
            assert_eq!(seen, format!("{at} {line} "), "for {input:?}");
        }
    }

    #[test]
    fn a_quoted_cell_left_open_is_failure_6_on_the_line_the_row_started_on() {
        let want = "line 7: failure 6: e.csv line 9: a quoted cell is not closed\n";
        for input in [
            &b"\""[..],
            b"\"abc",
            b"\"a\"\"",
            b"\"a\nb\nc,d",
            b"\"a,\"\"b",
        ] {
            let (o, seen) = cell(input);
            assert_eq!((o.status, seen.as_str()), (2, want), "for {input:?}");
        }
    }

    /// Reads rows of `input` until the end: each row shown as `show` shows a list, then its
    /// first line. Gives the outcome and what was shown.
    fn rows(input: &[u8]) -> (Outcome, String) {
        let end = Label(1);
        let mut after = Vec::new();
        for _ in 0..12 {
            after.extend([
                LoadSlot(A, 1),
                Push(A),
                AddressOf(A, Data::Text(0)),
                Push(A),
            ]);
            after.extend([Const(A, 7), Push(A), Call("csv_next".into())]);
            after.extend([Const(B, 0), Compare, JumpIf(Eq, end), Push(A)]);
            after.extend([
                AddressOf(A, Data::Desc(1)),
                Push(A),
                Call("show_value".into()),
            ]);
            after.extend([
                AddressOf(A, Data::Text(1)),
                Push(A),
                Call("out_text".into()),
            ]);
            after.extend([
                LoadSlot(A, 1),
                Load {
                    to: A,
                    base: A,
                    offset: 24,
                },
            ]);
            after.extend(say());
        }
        after.extend([Mark(end), Call("flush".into())]);
        let o = with_cursor(input, after);
        let shown = String::from_utf8(o.stdout.clone()).unwrap();
        let lines = String::from_utf8(o.stderr.clone()).unwrap();
        (o, format!("{shown}| {lines}"))
    }

    #[test]
    fn rows_end_at_a_line_feed_or_a_cr_lf_or_the_end_and_know_their_first_line() {
        let cases = [
            (
                "a,b\n1,2\n",
                "list of \"a\", \"b\"\nlist of \"1\", \"2\"\n| 1 2 ",
            ),
            (
                "a,b\n1,2",
                "list of \"a\", \"b\"\nlist of \"1\", \"2\"\n| 1 2 ",
            ),
            (
                "a,b\r\n1,2\r\n",
                "list of \"a\", \"b\"\nlist of \"1\", \"2\"\n| 1 2 ",
            ),
            ("", "| "),
            ("a\n", "list of \"a\"\n| 1 "),
            ("\n", "list of \"\"\n| 1 "),
            ("a\n\n", "list of \"a\"\nlist of \"\"\n| 1 2 "),
            (
                "a\n\nb",
                "list of \"a\"\nlist of \"\"\nlist of \"b\"\n| 1 2 3 ",
            ),
            ("a,\n", "list of \"a\", \"\"\n| 1 "),
            ("a,", "list of \"a\", \"\"\n| 1 "),
            (",", "list of \"\", \"\"\n| 1 "),
            (
                "a\n\"x\ny\"\nb\n",
                "list of \"a\"\nlist of \"x\ny\"\nlist of \"b\"\n| 1 2 4 ",
            ),
            (
                "a\r\n\"q\r\nz\"\r\nb",
                "list of \"a\"\nlist of \"q\r\nz\"\nlist of \"b\"\n| 1 2 4 ",
            ),
            ("a\rb,c\n", "list of \"a\rb\", \"c\"\n| 1 "),
        ];
        for (input, want) in cases {
            let (o, got) = rows(input.as_bytes());
            assert_eq!((o.status, got.as_str()), (0, want), "for {input:?}");
        }
    }

    #[test]
    fn an_unclosed_quote_names_the_line_its_row_started_on() {
        let (o, got) = rows(b"a\nb\n\"c\nd");
        let err = "line 7: failure 6: e.csv line 3: a quoted cell is not closed\n";
        assert_eq!(o.status, 2);
        assert_eq!(String::from_utf8(o.stderr).unwrap(), format!("1 2 {err}"));
        assert!(got.starts_with("list of \"a\"\nlist of \"b\"\n"));
    }
}
