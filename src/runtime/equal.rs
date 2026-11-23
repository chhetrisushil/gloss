//! Comparing two values of one type by its descriptor (see `ir::Desc`): a record field by
//! field, a list item by item. Nothing here recurses (lists hold records, records scalars).

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Ne};
use crate::ir::Reg::{self, A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

/// Loads the word at `base + offset` into `to`.
fn load(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(Load { to, base, offset });
}

/// Calls `callee` with this routine's three arguments, and returns its answer.
fn pass_on(s: &mut Asm, callee: &str) {
    for arg in [1, 2, 3] {
        s.push_arg(arg);
    }
    s.call(callee);
    s.i(Return);
}

/// `equal_scalar(x, y, desc)`: a number or truth by value, a text by its bytes.
fn equal_scalar() -> Function {
    let mut s = Asm::default();
    let text = s.label();
    s.i(LoadArg(A, 3));
    load(&mut s, A, A, 0);
    s.i(Const(B, 1));
    s.jump_if(Eq, text);
    s.i(LoadArg(A, 1));
    s.i(LoadArg(B, 2));
    s.i(Compare);
    s.i(SetIf(Eq));
    s.i(Return);
    s.i(Mark(text));
    s.push_arg(1);
    s.push_arg(2);
    s.call("text_equal");
    s.i(Return);
    s.finish("equal_scalar", 3, 0)
}

/// Pushes the item `index` (slot 1) of the list argument `arg`.
fn push_item(s: &mut Asm, arg: u32) {
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 3));
    s.i(LoadArg(B, arg));
    load(s, B, B, 16);
    s.i(Int(IntOp::Add));
    load(s, A, A, 0);
    s.i(Push(A));
}

/// `equal_fields(x, y, desc)`: two records of one type, field by field. Slots: 1 the field
/// index, 2 the field count.
fn equal_fields() -> Function {
    let mut s = Asm::default();
    let (again, differ, done) = (s.label(), s.label(), s.label());
    s.i(LoadArg(A, 3));
    load(&mut s, A, A, 16);
    s.i(StoreSlot(2, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    for arg in [1, 2] {
        s.i(LoadSlot(A, 1));
        s.i(Shift(Shift::Left, 3));
        s.i(LoadArg(B, arg));
        s.i(Int(IntOp::Add));
        load(&mut s, A, A, 0);
        s.i(Push(A));
    }
    // The field's descriptor sits at `desc + 32 + 16 * i`.
    s.i(LoadSlot(A, 1));
    s.i(Shift(Shift::Left, 4));
    s.i(LoadArg(B, 3));
    s.i(Int(IntOp::Add));
    load(&mut s, A, A, 32);
    s.i(Push(A));
    s.call("equal_scalar");
    s.i(Const(B, 0));
    s.jump_if(Eq, differ);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(Const(A, 1));
    s.i(Return);
    s.i(Mark(differ));
    s.finish("equal_fields", 3, 2)
}

/// `equal_list(x, y, desc)`: the counts, then item by item. Slots: index, count, item desc.
fn equal_list() -> Function {
    let mut s = Asm::default();
    let (again, done, differ, record, check) =
        (s.label(), s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 3));
    load(&mut s, A, A, 8);
    s.i(StoreSlot(3, A));
    s.i(LoadArg(A, 1));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(2, A));
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 0);
    s.i(LoadSlot(B, 2));
    s.jump_if(Ne, differ);
    s.i(Const(A, 0));
    s.i(StoreSlot(1, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, 1));
    s.i(LoadSlot(B, 2));
    s.jump_if(Ge, done);
    push_item(&mut s, 1);
    push_item(&mut s, 2);
    s.push_slot(3);
    s.i(LoadSlot(A, 3));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.call("equal_scalar");
    s.i(Jump(check));
    s.i(Mark(record));
    s.call("equal_fields");
    s.i(Mark(check));
    s.i(Const(B, 0));
    s.jump_if(Eq, differ);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(1, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(Const(A, 1));
    s.i(Return);
    s.i(Mark(differ));
    s.finish("equal_list", 3, 3)
}

/// `equal_values(x, y, desc)`: 1 when the two values of the type `desc` describes are equal.
fn equal_values() -> Function {
    let mut s = Asm::default();
    let (record, list) = (s.label(), s.label());
    s.i(LoadArg(A, 3));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.i(Const(B, 4));
    s.jump_if(Eq, list);
    pass_on(&mut s, "equal_scalar");
    s.i(Mark(record));
    pass_on(&mut s, "equal_fields");
    s.i(Mark(list));
    pass_on(&mut s, "equal_list");
    s.finish("equal_values", 3, 0)
}

pub(super) fn routines() -> Vec<Function> {
    vec![equal_scalar(), equal_fields(), equal_list(), equal_values()]
}

#[cfg(test)]
mod tests {
    use super::super::heap_tests::{Heap, W};
    use super::super::tests::{program, run};
    use crate::ir::Reg::A;
    use crate::ir::{Data, Desc, Inst::*, Module};

    /// Whether `equal_values` finds texts `x` and `y` equal.
    fn texts_equal(x: &[u8], y: &[u8]) -> bool {
        let body = vec![
            AddressOf(A, Data::Text(0)),
            Push(A),
            AddressOf(A, Data::Text(u32::from(x != y))),
            Push(A),
            AddressOf(A, Data::Desc(0)),
            Push(A),
            Call("equal_values".into()),
            Push(A),
            Call("out_number".into()),
            Call("flush".into()),
        ];
        let texts: &[&[u8]] = if x == y { &[x] } else { &[x, y] };
        let mut m = program(body, texts);
        m.desc(Desc::Text);
        run(&m).stdout == b"0.0001"
    }

    #[test]
    fn texts_are_equal_by_their_bytes() {
        assert!(texts_equal(b"Asha", b"Asha"));
        assert!(!texts_equal(b"Asha", b"Ashe"));
        assert!(!texts_equal(b"Asha", b"Ash"));
    }

    /// A record of a text, a number and a truth, and the descriptors of its type and lists.
    fn types() -> (Module, [u32; 3]) {
        let mut d = Module::default();
        let (number, text, truth) = (
            d.desc(Desc::Number),
            d.desc(Desc::Text),
            d.desc(Desc::Truth),
        );
        let fields = vec![
            ("n".into(), text),
            ("a".into(), number),
            ("t".into(), truth),
        ];
        let record = d.desc(Desc::Record {
            name: "E".into(),
            fields,
        });
        let (numbers, records) = (d.desc(Desc::List(number)), d.desc(Desc::List(record)));
        (d, [record, numbers, records])
    }

    fn records(a: [i64; 3], b: [i64; 3]) -> bool {
        let (d, [record, ..]) = types();
        let mut h = Heap::default();
        let mut make = |x: [i64; 3]| h.block(&[W::T(x[0] as u32), W::N(x[1]), W::N(x[2])]);
        let (x, y) = (make(a), make(b));
        h.call("equal_values", &[x, y], record, d) == b"0.0001"
    }

    #[test]
    fn records_are_equal_field_by_field() {
        assert!(records([2, 9, 1], [2, 9, 1]));
        assert!(!records([2, 9, 1], [2, 9, 0]));
        assert!(!records([2, 9, 1], [2, 8, 1]));
    }

    fn numbers(a: &[i64], b: &[i64]) -> bool {
        let (d, [_, list, _]) = types();
        let mut h = Heap::default();
        let items = |v: &[i64]| v.iter().map(|n| W::N(*n)).collect::<Vec<_>>();
        let (x, y) = (h.list(&items(a)), h.list(&items(b)));
        h.call("equal_values", &[x, y], list, d) == b"0.0001"
    }

    #[test]
    fn lists_are_equal_by_count_then_every_item() {
        assert!(numbers(&[1, 2, 3], &[1, 2, 3]));
        assert!(numbers(&[], &[]));
        assert!(!numbers(&[1, 2, 3], &[1, 2]));
        assert!(!numbers(&[1, 2], &[1, 2, 3]));
        assert!(!numbers(&[1, 2, 3], &[1, 2, 4]));
    }

    fn record_lists(a: [[i64; 3]; 2], b: [[i64; 3]; 2]) -> bool {
        let (d, [_, _, list]) = types();
        let mut h = Heap::default();
        let mut make = |l: [[i64; 3]; 2]| {
            let items: Vec<W> = l
                .iter()
                .map(|r| W::S(h.block(&[W::T(r[0] as u32), W::N(r[1]), W::N(r[2])])))
                .collect();
            h.list(&items)
        };
        let (x, y) = (make(a), make(b));
        h.call("equal_values", &[x, y], list, d) == b"0.0001"
    }

    #[test]
    fn lists_of_records_compare_every_field_of_every_record() {
        let base = [[2, 1, 1], [3, 2, 0]];
        assert!(record_lists(base, base));
        assert!(!record_lists(base, [[2, 1, 1], [3, 2, 1]]));
        assert!(!record_lists(base, [[2, 1, 1], [3, 5, 0]]));
    }
}
