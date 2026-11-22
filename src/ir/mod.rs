//! The Gloss IR: a target-neutral machine with two registers, `a` and `b`, numbered local
//! slots, and a stack. Backends translate it to machine code; tests interpret it.

/// One of the two registers. Numeric instructions work on `A` and `B` and leave the result
/// in `A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    A,
    B,
}

/// A jump target within a function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label(pub u32);

/// A signed comparison between `a` and `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Wrapping integer operations: `a = a op b`. `Div` and `Rem` are signed and truncating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
}

/// A shift of `a`: arithmetic left, arithmetic right, or logical right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Left,
    Right,
    RightLogical,
}

/// A primitive the host provides. The arguments were pushed left to right; `System`
/// consumes them and leaves the result in `a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    /// `Write(fd, addr, len)`: bytes written, or a negative error.
    Write,
    /// `Read(fd, addr, len)`: bytes read, 0 at end of input, negative on error.
    Read,
    /// `Map(len)`: the address of `len` fresh zeroed bytes, or 0.
    Map,
    /// `Exit(status)`: does not return.
    Exit,
    /// `StackSize()`: the soft stack limit in bytes, or -1 when unlimited.
    StackSize,
}

/// A runtime global, addressed with `Data::Global`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Global {
    HeapNext,
    HeapEnd,
    StackLimit,
    OutLen,
    OutBuf,
    InLen,
    InPos,
    InBuf,
}

/// Something whose address `AddressOf` can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    /// A read-only text constant, by index into `Module::texts`.
    Text(u32),
    /// A type descriptor, by index into `Module::descs`.
    Desc(u32),
    Global(Global),
}

/// One instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
    /// Marks where source line `n`'s code starts.
    Line(u32),
    Const(Reg, i64),
    /// Slots are numbered from 1.
    LoadSlot(Reg, u32),
    /// Stores a register into a slot; slots are numbered from 1.
    StoreSlot(u32, Reg),
    /// Arguments are numbered from 1, left to right.
    LoadArg(Reg, u32),
    Move {
        to: Reg,
        from: Reg,
    },
    Push(Reg),
    Pop(Reg),
    AddressOf(Reg, Data),
    StackPointer(Reg),
    Load {
        to: Reg,
        base: Reg,
        offset: i32,
    },
    Store {
        base: Reg,
        offset: i32,
        from: Reg,
    },
    LoadByte {
        to: Reg,
        base: Reg,
        offset: i32,
    },
    StoreByte {
        base: Reg,
        offset: i32,
        from: Reg,
    },
    /// `a = a + b`; jumps to `range` on overflow. The `Num*` instructions mean exactly the
    /// `Number` methods.
    NumAdd {
        range: Label,
    },
    /// `a = a - b`; jumps to `range` on overflow.
    NumSub {
        range: Label,
    },
    /// `a = -a`; jumps to `range` on overflow.
    NumNeg {
        range: Label,
    },
    /// `a = mul_round(a, b)`.
    NumMul {
        range: Label,
    },
    /// `a = div_round(a, b)`; division by zero jumps to `zero`, an out-of-range result to
    /// `range`.
    NumDiv {
        zero: Label,
        range: Label,
    },
    /// `a = rem(a, b)`; a zero divisor jumps to `zero`.
    NumRem {
        zero: Label,
    },
    /// `a = whole(a)`.
    NumWhole,
    /// `a = a op b`, wrapping.
    Int(IntOp),
    /// `r = r + n`.
    AddImm(Reg, i32),
    /// `a = a << n`, or `a >> n` (arithmetic or logical), with `n` from 0 to 63.
    Shift(Shift, u8),
    /// Compares `a` with `b`, for the next `JumpIf` or `SetIf`.
    Compare,
    /// Jumps when `a cond b`.
    JumpIf(Cond, Label),
    /// Sets `a` to 1 when `a cond b`, else 0.
    SetIf(Cond),
    Jump(Label),
    Mark(Label),
    /// The caller pushes the arguments left to right, then calls; the backend drops them
    /// (the callee's `params` says how many). The result is in `a`. `b` and the caller's
    /// slots survive the call; `a` does not.
    Call(String),
    /// Leaves the function with `a` as its result.
    Return,
    /// The first instruction of every function: reserves `n` zeroed 8-byte slots.
    Enter(u32),
    /// Placed just before a call: jumps to `fail` when the stack pointer minus `bytes` is
    /// below `Global::StackLimit` (unsigned), that is, when the callee's frame would not fit.
    StackCheck {
        bytes: u32,
        fail: Label,
    },
    System(Prim),
}
