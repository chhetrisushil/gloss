//! Writing text into the output buffer as the inside of a JSON string.

use super::asm::Asm;
use super::shown::{AT, CP, END, LEN, TMP, decode};
use crate::ir::Cond::{Ge, Gt, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp, Shift};

// Slots of `json_bytes` besides those `decode` uses.
const HIGH: u32 = 8;
const LOW: u32 = 9;

/// Writes the first `n` bytes of `word` (little-endian) through `buf_bytes`.
fn put_word(s: &mut Asm, word: i64, n: i64) {
    s.i(Const(A, word));
    put_a(s, n);
}

/// Writes the first `n` bytes of the word in `a` (little-endian) through `buf_bytes`. The
/// bytes live on the stack for the call, so nothing is allocated.
fn put_a(s: &mut Asm, n: i64) {
    s.i(Push(A));
    s.i(StackPointer(A));
    s.i(Push(A));
    s.i(Const(A, n));
    s.i(Push(A));
    s.call("buf_bytes");
    s.i(Pop(B));
}

/// `a` = the lower-case hex digit's byte for the nibble in `slot`.
fn hex_digit(s: &mut Asm, slot: u32) {
    s.i(LoadSlot(A, slot));
    s.i(Const(B, 9));
    s.i(Compare);
    s.i(SetIf(Gt));
    s.i(Const(B, i64::from(b'a' - b'0' - 10)));
    s.i(Int(IntOp::Mul));
    s.i(LoadSlot(B, slot));
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, i32::from(b'0')));
}

/// `json_bytes(addr, len)`: writes the bytes through `buf_bytes` as the inside of a JSON
/// string. `"` and `\` get a backslash, `\n`, `\r` and `\t` their short forms, every other
/// byte below 0x20 and 0x7F `\u00xx`; a valid UTF-8 sequence is copied; each byte that starts
/// none is `U+FFFD`. It never allocates. `decode` reads the bytes from argument 1.
fn json_bytes() -> Function {
    let mut s = Asm::default();
    let [again, done, bad, advance, plain, control] = [(); 6].map(|_| s.label());
    s.i(LoadArg(A, 2));
    s.i(StoreSlot(END, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(AT, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, END));
    s.jump_if(Ge, done);
    decode(&mut s, bad, 0);
    for (cp, word) in [
        (b'"', 0x225C),
        (b'\\', 0x5C5C),
        (b'\n', 0x6E5C),
        (b'\r', 0x725C),
        (b'\t', 0x745C),
    ] {
        let next = s.label();
        s.i(LoadSlot(A, CP));
        s.i(Const(B, i64::from(cp)));
        s.jump_if(Ne, next);
        put_word(&mut s, word, 2);
        s.i(Jump(advance));
        s.i(Mark(next));
    }
    s.i(LoadSlot(A, CP));
    s.i(Const(B, 0x20));
    s.jump_if(Lt, control);
    s.i(Const(B, 0x7F));
    s.jump_if(Ne, plain);
    s.i(Mark(control));
    s.i(LoadSlot(A, CP));
    s.i(Shift(Shift::Right, 4));
    s.i(StoreSlot(HIGH, A));
    s.i(LoadSlot(A, CP));
    s.i(Const(B, 0x0F));
    s.i(Int(IntOp::And));
    s.i(StoreSlot(LOW, A));
    hex_digit(&mut s, HIGH);
    s.i(Shift(Shift::Left, 32));
    s.i(StoreSlot(TMP, A));
    hex_digit(&mut s, LOW);
    s.i(Shift(Shift::Left, 40));
    s.i(LoadSlot(B, TMP));
    s.i(Int(IntOp::Or));
    s.i(Const(B, 0x3030_755C));
    s.i(Int(IntOp::Or));
    put_a(&mut s, 6);
    s.i(Jump(advance));
    s.i(Mark(plain));
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, AT));
    s.i(Int(IntOp::Add));
    s.i(Push(A));
    s.push_slot(LEN);
    s.call("buf_bytes");
    s.i(Jump(advance));
    // `decode` has set the length to 1 for a byte that starts no sequence.
    s.i(Mark(bad));
    put_word(&mut s, 0x00BD_BFEF, 3);
    s.i(Mark(advance));
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, LEN));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(AT, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.finish("json_bytes", 2, 9)
}

pub fn routines() -> Vec<Function> {
    vec![json_bytes()]
}
