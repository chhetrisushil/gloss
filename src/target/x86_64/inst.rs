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

/// Prints one instruction in Intel syntax.
pub fn text(x: &X) -> String {
    match x {
        X::MovRR(d, s) => format!("mov {}, {}", reg(*d), reg(*s)),
        X::MovRI(d, n) => format!("mov {}, {n}", reg(*d)),
        X::MovRM(d, m) => format!("mov {}, {}", reg(*d), mem(m)),
        X::MovMR(m, s) => format!("mov {}, {}", mem(m), reg(*s)),
        X::MovzxRM8(d, m) => format!("movzx {}, byte {}", reg32(*d), mem(m)),
        X::MovM8R(m, s) => format!("mov byte {}, {}", mem(m), reg8(*s)),
        X::Lea(d, m) => format!("lea {}, {}", reg(*d), mem(m)),
        X::Push(r) => format!("push {}", reg(*r)),
        X::Pop(r) => format!("pop {}", reg(*r)),
        X::Alu(op, d, s) => format!("{} {}, {}", alu(*op), reg(*d), reg(*s)),
        X::AluI(op, d, n) => format!("{} {}, {n}", alu(*op), reg(*d)),
        X::Imul(d, s) => format!("imul {}, {}", reg(*d), reg(*s)),
        X::ImulWide(r) => format!("imul {}", reg(*r)),
        X::Mul(r) => format!("mul {}", reg(*r)),
        X::Idiv(r) => format!("idiv {}", reg(*r)),
        X::Div(r) => format!("div {}", reg(*r)),
        X::Cqo => "cqo".to_string(),
        X::Neg(r) => format!("neg {}", reg(*r)),
        X::Test(a, b) => format!("test {}, {}", reg(*a), reg(*b)),
        X::Shl(r, n) => format!("shl {}, {n}", reg(*r)),
        X::Sar(r, n) => format!("sar {}, {n}", reg(*r)),
        X::Shr(r, n) => format!("shr {}, {n}", reg(*r)),
        X::Setcc(c, r) => format!("set{} {}", cc(*c), reg8(*r)),
        X::Jcc(c, l) => format!("j{} {l}", cc(*c)),
        X::Jmp(l) => format!("jmp {l}"),
        X::Call(l) => format!("call {l}"),
        X::Ret => "ret".to_string(),
        X::Syscall => "syscall".to_string(),
        X::Label(l) => format!("{l}:"),
        X::Comment(c) => format!("; {c}"),
    }
}

fn mem(m: &Mem) -> String {
    match m {
        Mem::Base(r, d) if *d < 0 => format!("[{}-{}]", reg(*r), -i64::from(*d)),
        Mem::Base(r, d) => format!("[{}+{d}]", reg(*r)),
        Mem::Rip(s) => format!("[rip+{}]", sym(s)),
    }
}

fn sym(s: &Sym) -> String {
    match s {
        Sym::Text(n) => format!("text_{n}"),
        Sym::Desc(n) => format!("desc_{n}"),
        Sym::Global(g) => global(*g).to_string(),
        Sym::Label(l) => l.clone(),
    }
}

fn global(g: Global) -> &'static str {
    match g {
        Global::HeapNext => "heap_next",
        Global::HeapEnd => "heap_end",
        Global::StackLimit => "stack_limit",
        Global::OutLen => "out_len",
        Global::OutBuf => "out_buf",
        Global::InLen => "in_len",
        Global::InPos => "in_pos",
        Global::InBuf => "in_buf",
    }
}

fn alu(op: AluOp) -> &'static str {
    match op {
        AluOp::Add => "add",
        AluOp::Sub => "sub",
        AluOp::And => "and",
        AluOp::Or => "or",
        AluOp::Xor => "xor",
        AluOp::Cmp => "cmp",
    }
}

fn cc(c: Cc) -> &'static str {
    match c {
        Cc::E => "e",
        Cc::Ne => "ne",
        Cc::L => "l",
        Cc::Le => "le",
        Cc::G => "g",
        Cc::Ge => "ge",
        Cc::B => "b",
        Cc::Ae => "ae",
        Cc::O => "o",
    }
}

fn reg(r: R) -> &'static str {
    match r {
        R::Rax => "rax",
        R::Rcx => "rcx",
        R::Rdx => "rdx",
        R::Rbx => "rbx",
        R::Rsp => "rsp",
        R::Rbp => "rbp",
        R::Rsi => "rsi",
        R::Rdi => "rdi",
        R::R8 => "r8",
        R::R9 => "r9",
        R::R10 => "r10",
        R::R11 => "r11",
        R::R12 => "r12",
        R::R13 => "r13",
        R::R14 => "r14",
        R::R15 => "r15",
    }
}

fn reg32(r: R) -> &'static str {
    match r {
        R::Rax => "eax",
        R::Rcx => "ecx",
        R::Rdx => "edx",
        R::Rbx => "ebx",
        R::Rsp => "esp",
        R::Rbp => "ebp",
        R::Rsi => "esi",
        R::Rdi => "edi",
        R::R8 => "r8d",
        R::R9 => "r9d",
        R::R10 => "r10d",
        R::R11 => "r11d",
        R::R12 => "r12d",
        R::R13 => "r13d",
        R::R14 => "r14d",
        R::R15 => "r15d",
    }
}

fn reg8(r: R) -> &'static str {
    match r {
        R::Rax => "al",
        R::Rcx => "cl",
        R::Rdx => "dl",
        R::Rbx => "bl",
        R::Rsp => "spl",
        R::Rbp => "bpl",
        R::Rsi => "sil",
        R::Rdi => "dil",
        R::R8 => "r8b",
        R::R9 => "r9b",
        R::R10 => "r10b",
        R::R11 => "r11b",
        R::R12 => "r12b",
        R::R13 => "r13b",
        R::R14 => "r14b",
        R::R15 => "r15b",
    }
}
