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
const SPECIAL: [u8; 4] = [b',', b'"', b'\r', b'\n'];

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

pub fn routines() -> Vec<Function> {
    vec![csv_byte(), csv_add(), csv_cell()]
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
