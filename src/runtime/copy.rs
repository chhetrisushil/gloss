//! Copying a value by its descriptor (see `ir::Desc`): a record gets a new block, a list a new
//! header and items with each record item copied too. A text is never copied (it is
//! immutable) and a number or truth is its own copy. Nothing here recurses.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Ne};
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

/// `copy_list(list, desc, line)`: a new list with room for at least 4 items (so appending to
/// the copy works at once), each record item copied. Slots: 1 the new header, 2 the index,
/// 3 the count, 4 the item descriptor, 5 the byte offset, 6 the item.
fn copy_list() -> Function {
    let mut s = Asm::default();
    let (room, again, done, plain) = (s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 1));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(3, A));
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 8);
    s.i(StoreSlot(4, A));
    s.i(LoadSlot(A, 3));
    s.i(Const(B, 4));
    s.jump_if(Ge, room);
    s.i(Const(A, 4));
    s.i(Mark(room));
    s.i(Push(A));
    s.push_arg(3);
    s.call("list_new");
    s.i(StoreSlot(1, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(2, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 2));
    s.i(LoadSlot(B, 3));
    s.jump_if(Ge, done);
    s.i(Shift(Shift::Left, 3));
    s.i(StoreSlot(5, A));
    s.i(LoadArg(B, 1));
    load(&mut s, B, B, 16);
    s.i(Int(IntOp::Add));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(6, A));
    s.i(LoadSlot(A, 4));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Ne, plain);
    s.push_slot(6);
    s.i(LoadSlot(A, 4));
    load(&mut s, A, A, 16);
    s.i(Push(A));
    s.push_arg(3);
    s.call("copy_record");
    s.i(StoreSlot(6, A));
    s.i(Mark(plain));
    s.i(LoadSlot(A, 1));
    load(&mut s, A, A, 16);
    s.i(LoadSlot(B, 5));
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, 6));
    store(&mut s, A, 0, B);
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(2, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(LoadSlot(B, 3));
    s.i(LoadSlot(A, 1));
    store(&mut s, A, 0, B);
    s.i(Return);
    s.finish("copy_list", 3, 6)
}

/// `copy_value(value, desc, line)`: a copy of the value of the type `desc` describes.
fn copy_value() -> Function {
    let mut s = Asm::default();
    let (record, list) = (s.label(), s.label());
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.i(Const(B, 4));
    s.jump_if(Eq, list);
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
    s.i(Mark(list));
    for arg in [1, 2, 3] {
        s.push_arg(arg);
    }
    s.call("copy_list");
    s.i(Return);
    s.finish("copy_value", 3, 0)
}

pub(super) fn routines() -> Vec<Function> {
    vec![copy_record(), copy_list(), copy_value()]
}

#[cfg(test)]
mod tests {
    use super::super::heap_tests::{Heap, W};
    use crate::ir::{Desc, Module};

    /// A record of a number, and the descriptors of it and of a list of it.
    fn types() -> (Module, [u32; 2]) {
        let mut d = Module::default();
        let number = d.desc(Desc::Number);
        let fields = vec![("age".into(), number)];
        let record = d.desc(Desc::Record {
            name: "P".into(),
            fields,
        });
        let list = d.desc(Desc::List(record));
        (d, [record, list])
    }

    #[test]
    fn a_copied_record_does_not_change_with_the_original() {
        let (d, [record, _]) = types();
        let mut h = Heap::default();
        let r = h.block(&[W::N(10_000)]);
        let shown = h.copy_then_poke(r, record, (r, 0, 20_000), d);
        assert_eq!(shown, b"P with age 1");
    }

    #[test]
    fn a_copied_list_copies_its_records_too() {
        let (d, [_, list]) = types();
        let mut h = Heap::default();
        let r = h.block(&[W::N(10_000)]);
        let l = h.list(&[W::S(r)]);
        let shown = h.copy_then_poke(l, list, (r, 0, 20_000), d);
        assert_eq!(shown, b"list of (P with age 1)");
    }
}
