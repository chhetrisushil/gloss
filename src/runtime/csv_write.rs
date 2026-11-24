//! Writing a table as CSV text: the header, then one line per record.

use super::asm::Asm;
use crate::ir::Cond::Lt;
use crate::ir::Reg::{self, A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

fn load(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(Load { to, base, offset });
}

fn store(s: &mut Asm, base: Reg, offset: i32, from: Reg) {
    s.i(Store { base, offset, from });
}

fn store_byte(s: &mut Asm, base: Reg, offset: i32, from: Reg) {
    s.i(StoreByte { base, offset, from });
}

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

pub fn routines() -> Vec<Function> {
    vec![csv_byte()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Inst::*;
    use crate::ir::Reg::{A, B};

    /// What a builder with room for 64 bytes holds after `bytes` go through `csv_byte`.
    pub(in crate::runtime) fn appended(bytes: &[u8]) -> Vec<u8> {
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
        for byte in bytes {
            body.extend([LoadSlot(A, 1), Push(A), Const(A, i64::from(*byte)), Push(A)]);
            body.extend([Const(A, 0), Push(A), Call("csv_byte".into())]);
        }
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
        let mut m = program(body, &[]);
        m.functions[0].slots = 1;
        m.functions[0].body[0] = Enter(1);
        let out = run(&m);
        assert_eq!(out.status, 0);
        out.stdout
    }

    #[test]
    fn bytes_are_appended_in_order() {
        assert_eq!(appended(b""), b"");
        assert_eq!(appended(b"a,\"b\n"), b"a,\"b\n");
    }

    #[test]
    fn a_builder_that_is_full_grows_and_keeps_every_byte() {
        let long: Vec<u8> = (0..1000).map(|i| b'a' + (i % 26) as u8).collect();
        assert_eq!(appended(&long), long);
    }
}
