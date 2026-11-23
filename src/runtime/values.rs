//! Showing a value by its type descriptor (see `ir::Desc`), on standard output.

use super::asm::Asm;
use super::{NO, QUOTE, YES};
use crate::ir::Cond::{Eq, Ge, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp};

/// Writes the runtime text `n` to standard output.
fn out_const(s: &mut Asm, n: u32) {
    s.push_text(n);
    s.call("out_text");
}

/// Writes the byte at index slot 1 of text argument 1.
fn put_current(s: &mut Asm) {
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.push_const(1);
    s.call("out_bytes");
}

/// `out_quoted(text)`: the text in double quotes, each `"` inside it doubled (as in source).
/// Slots: 1 the index, 2 the length.
fn out_quoted() -> Function {
    let mut s = Asm::default();
    let (again, done, plain) = (s.label(), s.label(), s.label());
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
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    put_current(&mut s);
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Const(B, i64::from(b'"')));
    s.jump_if(Ne, plain);
    put_current(&mut s);
    s.i(Mark(plain));
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    out_const(&mut s, QUOTE);
    s.finish("out_quoted", 1, 2)
}

/// `show_scalar(value, desc)`: a number, text or truth as a program shows it in a record.
fn show_scalar() -> Function {
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
    s.finish("show_scalar", 2, 0)
}

pub(super) fn routines() -> Vec<Function> {
    vec![out_quoted(), show_scalar()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Reg::A;
    use crate::ir::{Data, Desc, Inst, Inst::*};

    /// Shows the scalar that `value` loads into `a`, of type `desc`, and gives the output.
    fn shown(texts: &[&[u8]], value: Inst, desc: Desc) -> String {
        let body = vec![
            value,
            Push(A),
            AddressOf(A, Data::Desc(0)),
            Push(A),
            Call("show_scalar".into()),
            Call("flush".into()),
        ];
        let mut m = program(body, texts);
        m.desc(desc);
        String::from_utf8(run(&m).stdout).unwrap()
    }

    #[test]
    fn a_text_is_quoted_with_its_quotes_doubled() {
        let at = AddressOf(A, Data::Text(0));
        assert_eq!(shown(&[b"a\"b\"\""], at, Desc::Text), "\"a\"\"b\"\"\"\"\"");
        let empty = AddressOf(A, Data::Text(0));
        assert_eq!(shown(&[b""], empty, Desc::Text), "\"\"");
    }

    #[test]
    fn numbers_and_truths_show_as_a_program_shows_them() {
        assert_eq!(shown(&[], Const(A, -15000), Desc::Number), "-1.5");
        assert_eq!(shown(&[], Const(A, 1), Desc::Truth), "yes");
        assert_eq!(shown(&[], Const(A, 0), Desc::Truth), "no");
    }
}
