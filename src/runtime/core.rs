//! The entry routine, the allocator and the stack limit.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Gt, Le, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Global, Inst::*, IntOp, Prim};

/// The least the allocator maps at a time.
const PIECE: i64 = 64 << 20;
/// The stack size assumed when the limit is unlimited (`StackSize` gives -1).
const DEFAULT_STACK: i64 = 8 << 20;
/// The most of the stack the limit allows.
const MAX_STACK: i64 = 1 << 30;
/// How far above the real end of the stack the limit sits.
const STACK_MARGIN: i64 = 64 << 10;

/// `_start`: runs the program's `f_start` (every feature is named `f_<feature>`, so features
/// never clash with runtime routines).
fn start() -> Function {
    let mut s = Asm::default();
    s.call("heap_init");
    s.call("stack_init");
    s.call("f_start");
    s.call("flush");
    s.push_const(0);
    s.i(System(Prim::Exit));
    s.finish("_start", 0, 0)
}

fn heap_init() -> Function {
    let mut s = Asm::default();
    s.i(Const(A, 0));
    s.gstore(Global::HeapNext, A, B);
    s.gstore(Global::HeapEnd, A, B);
    s.finish("heap_init", 0, 0)
}

/// `alloc(bytes, line)`: the address of `bytes` zeroed bytes (at least 8, rounded up to a
/// multiple of 8). Slots: 1 the rounded size, 2 the size of a new piece, 3 the result.
fn alloc() -> Function {
    let mut s = Asm::default();
    let (nonzero, fits, big, mapped, bad) = (s.label(), s.label(), s.label(), s.label(), s.label());
    // A negative size, or one whose rounding would overflow, is out of memory.
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Lt, bad);
    s.i(Const(B, i64::MAX - 7));
    s.jump_if(Gt, bad);
    s.i(AddImm(A, 7));
    s.i(Const(B, -8));
    s.i(Int(IntOp::And));
    s.i(Const(B, 0));
    s.jump_if(Ne, nonzero);
    s.i(Const(A, 8));
    s.i(Mark(nonzero));
    s.i(StoreSlot(1, A));
    // It fits when the room left, `heap_end - heap_next`, is at least the size.
    s.gload(A, Global::HeapEnd);
    s.gload(B, Global::HeapNext);
    s.i(Int(IntOp::Sub));
    s.i(LoadSlot(B, 1));
    s.jump_if(Ge, fits);
    // The block does not fit: map a piece of `max(PIECE, size)` and start allocating there.
    s.i(LoadSlot(A, 1));
    s.i(Const(B, PIECE));
    s.jump_if(Ge, big);
    s.i(Move { to: A, from: B });
    s.i(Mark(big));
    s.i(StoreSlot(2, A));
    s.i(Push(A));
    s.i(System(Prim::Map));
    s.i(Const(B, 0));
    s.jump_if(Ne, mapped);
    s.i(Mark(bad));
    s.push_arg(2);
    s.call("fail_memory");
    s.i(Mark(mapped));
    s.gstore(Global::HeapNext, A, B);
    s.i(LoadSlot(B, 2));
    s.i(Int(IntOp::Add));
    s.gstore(Global::HeapEnd, A, B);
    s.i(Mark(fits));
    s.gload(A, Global::HeapNext);
    s.i(StoreSlot(3, A));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.gstore(Global::HeapNext, A, B);
    s.i(LoadSlot(A, 3));
    s.i(Return);
    s.finish("alloc", 2, 3)
}

fn fail_memory() -> Function {
    let mut s = Asm::default();
    s.push_arg(1);
    s.push_const(10);
    s.call("fail_begin");
    s.push_text(super::OUT_OF_MEMORY);
    s.call("err_text");
    s.call("fail_end");
    s.finish("fail_memory", 1, 0)
}

/// `stack_limit(size, top)`: the limit for a stack top `top` and a `StackSize` of `size`
/// (-1 for unlimited): `top - min(size, 1 GiB) + 64 KiB`, with `size` 8 MiB when unlimited.
fn stack_limit() -> Function {
    let mut s = Asm::default();
    let (known, capped) = (s.label(), s.label());
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Ge, known);
    s.i(Const(A, DEFAULT_STACK));
    s.i(Mark(known));
    s.i(Const(B, MAX_STACK));
    s.jump_if(Le, capped);
    s.i(Move { to: A, from: B });
    s.i(Mark(capped));
    s.i(Move { to: B, from: A });
    s.i(LoadArg(A, 2));
    s.i(Int(IntOp::Sub));
    s.i(AddImm(A, STACK_MARGIN as i32));
    s.i(Return);
    s.finish("stack_limit", 2, 0)
}

/// `stack_init`: sets the limit from the top of the initial stack. The kernel puts `argc`,
/// argv, envp and the auxiliary vector at the entry stack pointer (`Global::EntrySp`) and the
/// strings above them; `AT_EXECFN` (31) points at the executable's name, the highest string, so
/// its end rounded up to a page is the stack top, however big the environment is. Without
/// `AT_EXECFN` the stack pointer here is used instead, which may leave too small a margin.
/// Slots: 1 the cursor through the entry stack, 2 the stack top.
fn stack_init() -> Function {
    const AT_EXECFN: i64 = 31;
    let mut s = Asm::default();
    let (env, env_end, aux, found, scan, ended, none, have) = (
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
        s.label(),
    );
    // Skip `argc`, the argv pointers and their NULL: `8 * (argc + 2)` bytes.
    s.gload(A, Global::EntrySp);
    s.i(StoreSlot(1, A));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(AddImm(A, 2));
    s.i(Shift(crate::ir::Shift::Left, 3));
    s.i(LoadSlot(B, 1));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(1, A));
    // Skip the envp pointers and their NULL.
    s.i(Mark(env));
    s.i(LoadSlot(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 0));
    s.jump_if(Eq, env_end);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 8));
    s.i(StoreSlot(1, A));
    s.i(Jump(env));
    s.i(Mark(env_end));
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 8));
    s.i(StoreSlot(1, A));
    // Walk the (type, value) pairs to `AT_EXECFN` or the `AT_NULL` that ends them.
    s.i(Mark(aux));
    s.i(LoadSlot(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 0));
    s.jump_if(Eq, none);
    s.i(Const(B, AT_EXECFN));
    s.jump_if(Eq, found);
    s.i(LoadSlot(A, 1));
    s.i(AddImm(A, 16));
    s.i(StoreSlot(1, A));
    s.i(Jump(aux));
    // The name's end (past its NUL), rounded up to a page.
    s.i(Mark(found));
    s.i(LoadSlot(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(StoreSlot(2, A));
    s.i(Mark(scan));
    s.i(LoadSlot(A, 2));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Const(B, 0));
    s.jump_if(Eq, ended);
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(2, A));
    s.i(Jump(scan));
    s.i(Mark(ended));
    s.i(LoadSlot(A, 2));
    s.i(AddImm(A, 1 + 4095));
    s.i(Const(B, -4096));
    s.i(Int(IntOp::And));
    s.i(StoreSlot(2, A));
    s.i(Jump(have));
    s.i(Mark(none));
    s.i(StackPointer(A));
    s.i(StoreSlot(2, A));
    s.i(Mark(have));
    s.i(System(Prim::StackSize));
    s.i(Push(A));
    s.push_slot(2);
    s.call("stack_limit");
    s.gstore(Global::StackLimit, A, B);
    s.finish("stack_init", 0, 2)
}

pub fn routines() -> Vec<Function> {
    vec![
        start(),
        heap_init(),
        alloc(),
        fail_memory(),
        stack_limit(),
        stack_init(),
    ]
}
