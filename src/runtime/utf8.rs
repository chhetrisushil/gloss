//! Checking that a text is UTF-8 (RFC 3629): `utf8_valid_up_to`.

use super::asm::Asm;
use crate::ir::Cond::{self, Ge, Gt, Lt, Ne};
use crate::ir::Function;
use crate::ir::Inst::*;
use crate::ir::Reg::{A, B};
use crate::ir::{IntOp, Label};

// Slots of `utf8_valid_up_to`.
const AT: u32 = 1;
const END: u32 = 2;
const BYTE: u32 = 3;
const EXTRA: u32 = 4;
const LOW: u32 = 5;
const HIGH: u32 = 6;
const K: u32 = 7;

/// The byte of the text (argument 1) at the index in `a`, into `a`.
fn byte_at(s: &mut Asm) {
    s.i(LoadArg(B, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
}

/// When the lead byte, in `BYTE`, does not satisfy `skip` against `n`, it starts a sequence
/// with these constants, and the code goes on at `checked`.
fn pick(s: &mut Asm, skip: Cond, n: i64, seq: (i64, i64, i64), checked: Label) {
    let next = s.label();
    s.i(LoadSlot(A, BYTE));
    s.i(Const(B, n));
    s.jump_if(skip, next);
    for (slot, n) in [(EXTRA, seq.0), (LOW, seq.1), (HIGH, seq.2)] {
        s.i(Const(A, n));
        s.i(StoreSlot(slot, A));
    }
    s.i(Jump(checked));
    s.i(Mark(next));
}

/// Jumps to `bad` unless the byte at the index in `a` lies in `low..=high`.
fn within(s: &mut Asm, low: i64, high: i64, bad: Label) {
    byte_at(s);
    s.i(Const(B, low));
    s.jump_if(Lt, bad);
    s.i(Const(B, high));
    s.jump_if(Gt, bad);
}

/// `a = slot x`, `b = slot y`.
fn load2(s: &mut Asm, x: u32, y: u32) {
    s.i(LoadSlot(A, x));
    s.i(LoadSlot(B, y));
}

/// Adds `n` to the slot.
fn bump(s: &mut Asm, slot: u32, n: i32) {
    s.i(LoadSlot(A, slot));
    s.i(AddImm(A, n));
    s.i(StoreSlot(slot, A));
}

/// `utf8_valid_up_to(text)`: the length of the longest prefix of `text` that is UTF-8, which
/// is the length of the text when all of it is. A sequence cut short by the end of the text,
/// an overlong form, a surrogate or a value above U+10FFFF is invalid from its lead byte.
/// It never reads past the length in the text. `K` is the offset being checked.
fn utf8_valid_up_to() -> Function {
    let mut s = Asm::default();
    let [again, done, checked, rest, more, ok] = [(); 6].map(|_| s.label());
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(END, A));
    s.i(Mark(again));
    load2(&mut s, AT, END);
    s.jump_if(Ge, done);
    s.i(LoadSlot(A, AT));
    byte_at(&mut s);
    s.i(StoreSlot(BYTE, A));
    // A byte below 0x80 is a character; one from 0x80 to 0xC1 cannot start one.
    s.i(Const(B, 0x80));
    s.jump_if(Ge, rest);
    bump(&mut s, AT, 1);
    s.i(Jump(again));
    s.i(Mark(rest));
    s.i(LoadSlot(A, BYTE));
    s.i(Const(B, 0xC2));
    s.jump_if(Lt, done);
    for (skip, lead, seq) in [
        (Ge, 0xE0, (1, 0x80, 0xBF)),
        (Ne, 0xE0, (2, 0xA0, 0xBF)),
        (Ne, 0xED, (2, 0x80, 0x9F)),
        (Ge, 0xF0, (2, 0x80, 0xBF)),
        (Ne, 0xF0, (3, 0x90, 0xBF)),
        (Ge, 0xF4, (3, 0x80, 0xBF)),
        (Ne, 0xF4, (3, 0x80, 0x8F)),
    ] {
        pick(&mut s, skip, lead, seq, checked);
    }
    s.i(Jump(done));
    s.i(Mark(checked));
    // The whole sequence must lie inside the text.
    load2(&mut s, AT, EXTRA);
    s.i(Int(IntOp::Add));
    s.i(LoadSlot(B, END));
    s.jump_if(Ge, done);
    // The first continuation byte has the range the lead byte gave.
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, 1));
    byte_at(&mut s);
    s.i(LoadSlot(B, LOW));
    s.jump_if(Lt, done);
    s.i(LoadSlot(B, HIGH));
    s.jump_if(Gt, done);
    // Any others are plain continuation bytes.
    s.i(Const(A, 2));
    s.i(StoreSlot(K, A));
    s.i(Mark(more));
    load2(&mut s, K, EXTRA);
    s.jump_if(Gt, ok);
    load2(&mut s, AT, K);
    s.i(Int(IntOp::Add));
    within(&mut s, 0x80, 0xBF, done);
    bump(&mut s, K, 1);
    s.i(Jump(more));
    s.i(Mark(ok));
    load2(&mut s, AT, EXTRA);
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(AT, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.i(LoadSlot(A, AT));
    s.i(Return);
    s.finish("utf8_valid_up_to", 1, 7)
}

pub fn routines() -> Vec<Function> {
    vec![utf8_valid_up_to()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::{Data, Inst::*, Reg::A};

    /// What `utf8_valid_up_to` gives for `bytes`, which sit in memory as a text followed by
    /// four continuation bytes, so a read past the end would look like part of a character.
    fn valid_up_to(bytes: &[u8]) -> usize {
        let blob = [&(bytes.len() as u64).to_le_bytes()[..], bytes, &[0x80; 4]].concat();
        let body = vec![
            AddressOf(A, Data::Text(0)),
            AddImm(A, 8),
            Push(A),
            Call("utf8_valid_up_to".into()),
            Push(A),
            Call("err_int".into()),
        ];
        let o = run(&program(body, &[&blob]));
        assert_eq!(o.status, 0);
        String::from_utf8(o.stderr).unwrap().parse().unwrap()
    }

    /// Byte strings in hex: valid ones at the limits of each length, overlong forms,
    /// surrogates, values above U+10FFFF, bytes that cannot lead, and cut-short sequences.
    const TABLE: &str = "
        00 41 7F C280 DFBF E0A080 EFBFBF F0908080 F48FBFBF 41C3A9E282AC F09F9880
        80 BF C0 C1 F5 FF C0AF C180 C2 C27F C2C0 DF E0 E09F80 E0A07F E0A0C0 E0A0
        ED9FBF EDA080 EDBFBF EE8080 EF E18080 E180 E1807F E1C080 F0 F08F80 F0908080
        F09080 F09080C0 F48F80 F4908080 F4BFBFBF F5808080 F8888080 FF8080
        41 C3 4142C3 41E282 4142F09F98 61C3 28 C328 E28228 F0289CBC 6180 61FF62
    ";

    #[test]
    fn the_check_agrees_with_std_on_a_table_of_byte_strings() {
        for hex in TABLE.split_whitespace() {
            let digits: Vec<u8> = hex
                .bytes()
                .map(|d| (d as char).to_digit(16).unwrap() as u8)
                .collect();
            let bytes: Vec<u8> = digits
                .chunks(2)
                .map(|c| c[0] * 16 + c.get(1).copied().unwrap_or(0))
                .collect();
            let want = std::str::from_utf8(&bytes).map_or_else(|e| e.valid_up_to(), |s| s.len());
            assert_eq!(valid_up_to(&bytes), want, "for {bytes:02X?}");
        }
    }
}
