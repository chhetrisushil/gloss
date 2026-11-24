//! Reading CSV text (RFC 4180): a cursor over the text that gives one row at a time.

use super::asm::Asm;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*};

const CURSOR_BYTES: i64 = 32;
// Words of a cursor: the text, the position in it (at 8), the line the next row starts on, and
// the line the row being read started on.
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

pub fn routines() -> Vec<Function> {
    vec![csv_cursor(), fail_row()]
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
}
