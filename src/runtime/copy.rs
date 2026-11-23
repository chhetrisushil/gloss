//! Copying a value by its descriptor (see `ir::Desc`): a record gets a new block, a list a new
//! header and items with each record item copied too. A text is never copied (it is
//! immutable) and a number or truth is its own copy. Nothing here recurses.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge};
use crate::ir::Reg::{self, A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

/// Loads the word at `base + offset` into `to`.
fn load(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(Load { to, base, offset });
}

/// Stores `from` at `base + offset`.
fn store(s: &mut Asm, base: Reg, offset: i32, from: Reg) {
    s.i(Store { base, offset, from });
}

/// `copy_record(record, fields, line)`: a new block with the same fields. Slots: 1 the new
/// block, 2 the field index, 3 its byte offset, 4 the field.
fn copy_record() -> Function {
    let mut s = Asm::default();
    let (again, done) = (s.label(), s.label());
    s.i(LoadArg(A, 2));
    s.i(Shift(Shift::Left, 3));
    s.i(Push(A));
    s.push_arg(3);
    s.call("alloc");
    s.i(StoreSlot(1, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(LoadArg(B, 2));
    s.jump_if(Ge, done);
    s.i(Shift(Shift::Left, 3));
    s.i(StoreSlot(3, A));
    s.i(LoadArg(B, 1));
    s.i(Int(IntOp::Add));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(4, A));
    s.i(LoadSlot(A, 3));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, 4));
    store(&mut s, A, 0, B);
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(LoadSlot(A, 1));
    s.i(Return);
    s.finish("copy_record", 3, 4)
}

/// `copy_value(value, desc, line)`: a copy of the value of the type `desc` describes.
fn copy_value() -> Function {
    let mut s = Asm::default();
    let record = s.label();
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.i(LoadArg(A, 1));
    s.i(Return);
    s.i(Mark(record));
    s.push_arg(1);
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 16);
    s.i(Push(A));
    s.push_arg(3);
    s.call("copy_record");
    s.i(Return);
    s.finish("copy_value", 3, 0)
}

pub(super) fn routines() -> Vec<Function> {
    vec![copy_record(), copy_value()]
}

#[cfg(test)]
mod tests {
    use super::super::heap_tests::{Heap, W};
    use crate::ir::{Desc, Module};

    /// A record of a number, and its descriptor.
    fn types() -> (Module, u32) {
        let mut d = Module::default();
        let number = d.desc(Desc::Number);
        let fields = vec![("age".into(), number)];
        let record = d.desc(Desc::Record {
            name: "P".into(),
            fields,
        });
        (d, record)
    }

    #[test]
    fn a_copied_record_does_not_change_with_the_original() {
        let (d, record) = types();
        let mut h = Heap::default();
        let r = h.block(&[W::N(10_000)]);
        let shown = h.copy_then_poke(r, record, (r, 0, 20_000), d);
        assert_eq!(shown, b"P with age 1");
    }
}
