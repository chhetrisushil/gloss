//! Typed x86-64 instructions and their Intel-syntax text.

use crate::ir::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum R {
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cc {
    E,
    Ne,
    L,
    Le,
    G,
    Ge,
    B,
    Ae,
    O,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AluOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Cmp,
}

/// A memory operand: `[r + disp]` or `[rip + symbol]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mem {
    Base(R, i32),
    Rip(Sym),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sym {
    Text(u32),
    Desc(u32),
    Global(Global),
    Label(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum X {
    MovRR(R, R),
    MovRI(R, i64),
    MovRM(R, Mem),
    MovMR(Mem, R),
    MovzxRM8(R, Mem),
    MovM8R(Mem, R),
    Lea(R, Mem),
    Push(R),
    Pop(R),
    Alu(AluOp, R, R),
    AluI(AluOp, R, i32),
    Imul(R, R),
    ImulWide(R),
    Mul(R),
    Idiv(R),
    Div(R),
    Cqo,
    Neg(R),
    Test(R, R),
    Shl(R, u8),
    Sar(R, u8),
    Shr(R, u8),
    Setcc(Cc, R),
    Jcc(Cc, String),
    Jmp(String),
    Call(String),
    Ret,
    Syscall,
    Label(String),
    Comment(String),
}
