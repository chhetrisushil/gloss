//! The number routines `Num` multiplication, division and remainder call, written in `X`.
//!
//! Each takes `a` (`rax`) and `b` (`rcx`) and returns the result in `rax` and a status in
//! `rdx`: 0 ok, 1 out of range, 2 divide by zero. They keep `rcx` and clobber only `rdx`,
//! `rsi`, `rdi` and `r8`-`r11`. They equal `Number::mul_round`, `div_round` and `rem`: the
//! rounding works on the 128-bit magnitude, half away from zero, and the high half is
//! divided first so `div` never traps.

use super::inst::{AluOp, Cc, R, X};
use crate::number::SCALE;

pub const MULTIPLY: &str = "multiply_number";
pub const DIVIDE: &str = "divide_number";
pub const REMAINDER: &str = "remainder_number";
/// Shared tail: `r9:rax` is the magnitude of the quotient and `r10` its sign (0 or 1).
const FINISH: &str = "number_finish";
const RANGE: &str = "number_range";
const ZERO: &str = "number_zero";

fn label(name: &str) -> X {
    X::Label(name.to_string())
}

fn jcc(c: Cc, name: &str) -> X {
    X::Jcc(c, name.to_string())
}

fn jmp(name: &str) -> X {
    X::Jmp(name.to_string())
}

fn clear(r: R) -> X {
    X::Alu(AluOp::Xor, r, r)
}

/// Every helper, ready to append after the lowered functions.
pub fn helpers() -> Vec<X> {
    let mut xs = Vec::new();
    multiply(&mut xs);
    divide(&mut xs);
    remainder(&mut xs);
    finish(&mut xs);
    xs
}

/// `r9:rax = (rdx:rax) / 10000` for the unsigned 128-bit `rdx:rax`, high half first.
fn divide_by_scale(out: &mut Vec<X>) {
    out.extend([
        X::MovRI(R::Rsi, SCALE),
        X::MovRR(R::R8, R::Rax),
        X::MovRR(R::Rax, R::Rdx),
        clear(R::Rdx),
        X::Div(R::Rsi),
        X::MovRR(R::R9, R::Rax),
        X::MovRR(R::Rax, R::R8),
        X::Div(R::Rsi),
    ]);
}

fn multiply(out: &mut Vec<X>) {
    out.extend([
        label(MULTIPLY),
        X::ImulWide(R::Rcx),
        X::MovRR(R::R10, R::Rdx),
        X::Shr(R::R10, 63),
        X::Test(R::R10, R::R10),
        jcc(Cc::E, "multiply_magnitude"),
        // Negate the 128-bit product: -hi, less one more unless the low half is 0.
        X::Neg(R::Rdx),
        X::Test(R::Rax, R::Rax),
        jcc(Cc::E, "multiply_low"),
        X::AluI(AluOp::Sub, R::Rdx, 1),
        label("multiply_low"),
        X::Neg(R::Rax),
        label("multiply_magnitude"),
        // Add half the scale across the pair, so the floor of the quotient rounds.
        X::AluI(AluOp::Add, R::Rax, (SCALE / 2) as i32),
        jcc(Cc::Ae, "multiply_carried"),
        X::AluI(AluOp::Add, R::Rdx, 1),
        label("multiply_carried"),
    ]);
    divide_by_scale(out);
    out.push(jmp(FINISH));
}

fn divide(out: &mut Vec<X>) {
    out.extend([
        label(DIVIDE),
        X::Test(R::Rcx, R::Rcx),
        jcc(Cc::E, ZERO),
        // r8, r9: the signs as 0 or -1. rax = |a|, r11 = |b|; r10 = the quotient's sign.
        X::MovRR(R::R8, R::Rax),
        X::Sar(R::R8, 63),
        X::Alu(AluOp::Xor, R::Rax, R::R8),
        X::Alu(AluOp::Sub, R::Rax, R::R8),
        X::MovRR(R::R9, R::Rcx),
        X::Sar(R::R9, 63),
        X::MovRR(R::R11, R::Rcx),
        X::Alu(AluOp::Xor, R::R11, R::R9),
        X::Alu(AluOp::Sub, R::R11, R::R9),
        X::MovRR(R::R10, R::R8),
        X::Alu(AluOp::Xor, R::R10, R::R9),
        X::AluI(AluOp::And, R::R10, 1),
        // rdx:rax = |a| * 10000, divided by |b| with the high half first.
        X::MovRI(R::Rsi, SCALE),
        X::Mul(R::Rsi),
        X::MovRR(R::R8, R::Rax),
        X::MovRR(R::Rax, R::Rdx),
        clear(R::Rdx),
        X::Div(R::R11),
        X::MovRR(R::R9, R::Rax),
        X::MovRR(R::Rax, R::R8),
        X::Div(R::R11),
        // Round up when the remainder is at least half the divisor: r >= |b| - r.
        X::MovRR(R::Rsi, R::R11),
        X::Alu(AluOp::Sub, R::Rsi, R::Rdx),
        X::Alu(AluOp::Cmp, R::Rdx, R::Rsi),
        jcc(Cc::B, FINISH),
        X::AluI(AluOp::Add, R::Rax, 1),
        jcc(Cc::Ae, FINISH),
        X::AluI(AluOp::Add, R::R9, 1),
        jmp(FINISH),
    ]);
}

/// The remainder keeps the dividend's sign. A divisor of -1 is answered before `idiv`,
/// which traps on `i64::MIN / -1`.
fn remainder(out: &mut Vec<X>) {
    out.extend([
        label(REMAINDER),
        X::Test(R::Rcx, R::Rcx),
        jcc(Cc::E, ZERO),
        X::AluI(AluOp::Cmp, R::Rcx, -1),
        jcc(Cc::E, "remainder_minus_one"),
        X::Cqo,
        X::Idiv(R::Rcx),
        X::MovRR(R::Rax, R::Rdx),
        clear(R::Rdx),
        X::Ret,
        label("remainder_minus_one"),
        clear(R::Rax),
        clear(R::Rdx),
        X::Ret,
    ]);
}

/// Range-checks the quotient `r9:rax` and applies the sign in `r10`: the magnitude may
/// reach `i64::MAX`, or 2^63 when negative.
fn finish(out: &mut Vec<X>) {
    out.extend([
        label(FINISH),
        X::Test(R::R9, R::R9),
        jcc(Cc::Ne, RANGE),
        X::MovRI(R::Rsi, i64::MAX),
        X::Alu(AluOp::Add, R::Rsi, R::R10),
        X::Alu(AluOp::Cmp, R::Rsi, R::Rax),
        jcc(Cc::B, RANGE),
        X::Test(R::R10, R::R10),
        jcc(Cc::E, "number_signed"),
        X::Neg(R::Rax),
        label("number_signed"),
        clear(R::Rdx),
        X::Ret,
        label(RANGE),
        X::MovRI(R::Rdx, 1),
        X::Ret,
        label(ZERO),
        X::MovRI(R::Rdx, 2),
        X::Ret,
    ]);
}
