//! Every `X` variant over a representative set of registers, operands and labels.

use gloss::ir::Global;
use gloss::target::x86_64::inst::{AluOp, Cc, Mem, R, Sym, X};

const REGS: [R; 8] = [
    R::Rax,
    R::Rcx,
    R::Rsp,
    R::Rbp,
    R::R12,
    R::R13,
    R::Rsi,
    R::R15,
];
const DISPS: [i32; 5] = [0, -16, 8, i32::MIN, i32::MAX];
const OPS: [AluOp; 6] = [
    AluOp::Add,
    AluOp::Sub,
    AluOp::And,
    AluOp::Or,
    AluOp::Xor,
    AluOp::Cmp,
];
const CCS: [Cc; 9] = [
    Cc::E,
    Cc::Ne,
    Cc::L,
    Cc::Le,
    Cc::G,
    Cc::Ge,
    Cc::B,
    Cc::Ae,
    Cc::O,
];

fn mems() -> Vec<Mem> {
    let mut v: Vec<Mem> = REGS
        .iter()
        .flat_map(|&r| DISPS.iter().map(move |&d| Mem::Base(r, d)))
        .collect();
    v.push(Mem::Rip(Sym::Text(3)));
    v.push(Mem::Rip(Sym::Desc(1)));
    v.push(Mem::Rip(Sym::Global(Global::OutBuf)));
    v
}

pub fn sweep() -> Vec<X> {
    let mut xs = vec![X::Comment("sweep".into()), X::Label("start".into())];
    let mems = mems();
    for &d in &REGS {
        for &s in &REGS {
            xs.push(X::MovRR(d, s));
            xs.push(X::Imul(d, s));
            xs.push(X::Test(d, s));
            xs.extend(OPS.iter().map(|&op| X::Alu(op, d, s)));
        }
        let imms = [0, 1, -1, i64::MIN, i64::MAX, 0x8000_0000, -0x8000_0001];
        xs.extend(imms.iter().map(|&n| X::MovRI(d, n)));
        for n in [0, -16, 8, i32::MIN, i32::MAX] {
            xs.extend(OPS.iter().map(|&op| X::AluI(op, d, n)));
        }
        for n in [0, 1, 63] {
            xs.extend([X::Shl(d, n), X::Sar(d, n), X::Shr(d, n)]);
        }
        xs.extend([
            X::Push(d),
            X::Pop(d),
            X::ImulWide(d),
            X::Mul(d),
            X::Idiv(d),
            X::Div(d),
        ]);
        xs.push(X::Neg(d));
        xs.extend(CCS.iter().map(|&c| X::Setcc(c, d)));
        for m in &mems {
            xs.extend([
                X::MovRM(d, m.clone()),
                X::MovMR(m.clone(), d),
                X::MovzxRM8(d, m.clone()),
                X::MovM8R(m.clone(), d),
                X::Lea(d, m.clone()),
            ]);
        }
    }
    xs.extend([X::Cqo, X::Syscall]);
    xs.push(X::Label("back".into()));
    xs.push(X::Ret);
    xs.push(X::Jmp("back".into()));
    xs.push(X::Call("back".into()));
    xs.push(X::Jmp("start".into()));
    xs.extend(CCS.iter().map(|&c| X::Jcc(c, "back".into())));
    xs.extend(CCS.iter().map(|&c| X::Jcc(c, "fwd".into())));
    xs.extend([X::Jmp("fwd".into()), X::Call("fwd".into())]);
    xs.push(X::Label("fwd".into()));
    xs.push(X::Ret);
    xs
}
