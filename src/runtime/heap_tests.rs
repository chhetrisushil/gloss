//! Test support: values built in the heap by IR.

use super::tests::{program, run};
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Inst, Inst::*, Module};

/// A word of a block: a number or truth, a runtime text by index, or the block in a slot.
pub(super) enum W {
    N(i64),
    T(u32),
    S(u32),
}

/// The instructions that build the blocks; slot `n` holds the `n`th block.
#[derive(Default)]
pub(super) struct Heap {
    body: Vec<Inst>,
    slots: u32,
}

impl Heap {
    /// A block of the words, in order; the slot that holds it.
    pub fn block(&mut self, words: &[W]) -> u32 {
        self.slots += 1;
        let slot = self.slots;
        let size = 8 * words.len().max(1) as i64;
        self.body
            .extend([Const(A, size), Push(A), Const(A, 0), Push(A)]);
        self.body.extend([Call("alloc".into()), StoreSlot(slot, A)]);
        for (i, w) in words.iter().enumerate() {
            self.body.push(match w {
                W::N(n) => Const(B, *n),
                W::T(t) => AddressOf(B, Data::Text(*t)),
                W::S(s) => LoadSlot(B, *s),
            });
            let store = Store {
                base: A,
                offset: 8 * i as i32,
                from: B,
            };
            self.body.extend([LoadSlot(A, slot), store]);
        }
        slot
    }

    /// A list: the header `[count, capacity, items]` and its items.
    pub fn list(&mut self, items: &[W]) -> u32 {
        let block = self.block(items);
        let n = items.len() as i64;
        self.block(&[W::N(n), W::N(n), W::S(block)])
    }

    /// Copies the value in slot `value` (type `desc`), then sets word `offset` of the block in
    /// slot `poke` to the number `n`, then shows the copy: the output tells a copy from an alias.
    pub fn copy_then_poke(
        mut self,
        value: u32,
        desc: u32,
        poke: (u32, i32, i64),
        d: Module,
    ) -> Vec<u8> {
        let at = AddressOf(A, Data::Desc(desc));
        self.slots += 1;
        let copy = self.slots;
        self.body
            .extend([LoadSlot(A, value), Push(A), at.clone(), Push(A)]);
        self.body.extend([
            Const(A, 0),
            Push(A),
            Call("copy_value".into()),
            StoreSlot(copy, A),
        ]);
        let (block, offset, n) = poke;
        self.body.extend([Const(B, n), LoadSlot(A, block)]);
        self.body.extend([
            Store {
                base: A,
                offset,
                from: B,
            },
            LoadSlot(A, copy),
            Push(A),
        ]);
        self.body.extend([at, Push(A), Call("show_value".into())]);
        self.call("flush", &[], desc, d)
    }

    /// Calls `routine` on the blocks in `slots` and the descriptor `desc`; the output.
    pub fn call(mut self, routine: &str, slots: &[u32], desc: u32, descs: Module) -> Vec<u8> {
        for s in slots {
            self.body.extend([LoadSlot(A, *s), Push(A)]);
        }
        self.body.extend([AddressOf(A, Data::Desc(desc)), Push(A)]);
        self.body.push(Call(routine.into()));
        if routine == "equal_values" {
            self.body.extend([Push(A), Call("out_number".into())]);
        }
        self.body.push(Call("flush".into()));
        let mut m = program(self.body, &[b"a\"b", b"", b"x", b"y"]);
        m.functions[0].slots = self.slots;
        m.functions[0].body[0] = Enter(self.slots);
        m.descs = descs.descs;
        run(&m).stdout
    }
}
