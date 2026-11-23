//! Writing a text into a failure message, cut and with invisible characters spelled out.

use super::asm::Asm;
use crate::chars::unshowable;
use crate::ir::Cond::Ge;
use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst::*, IntOp};

/// The most characters of a text a message shows; the constants after it are slots.
const SHOWN_CHARS: i64 = 40;
const AT: u32 = 1;
const END: u32 = 2;
const COUNT: u32 = 3;
const CP: u32 = 4;
const LEN: u32 = 5;
const K: u32 = 6;
const TMP: u32 = 7;

/// The byte of text argument 1 at the index in `a`, into `a` (clobbers `b`).
fn byte_at(s: &mut Asm) {
    s.i(Move { to: B, from: A });
    s.i(LoadArg(A, 1));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
}

/// `slot = slot op n`.
fn update(s: &mut Asm, slot: u32, op: IntOp, n: i64) {
    s.i(LoadSlot(A, slot));
    s.i(Const(B, n));
    s.i(Int(op));
    s.i(StoreSlot(slot, A));
}

/// The code point at `AT` into `CP`, its length in bytes into `LEN`.
fn decode(s: &mut Asm) {
    let [two, three, four, rest, more, done] = [(); 6].map(|_| s.label());
    s.i(LoadSlot(A, AT));
    byte_at(s);
    s.i(StoreSlot(CP, A));
    for (limit, len, mask, next) in [
        (0x80, 1, 0x7F, two),
        (0xE0, 2, 0x1F, three),
        (0xF0, 3, 0x0F, four),
    ] {
        s.i(LoadSlot(A, CP));
        s.i(Const(B, limit));
        s.jump_if(Ge, next);
        s.i(Const(A, len));
        s.i(StoreSlot(LEN, A));
        update(s, CP, IntOp::And, mask);
        s.i(Jump(rest));
        s.i(Mark(next));
    }
    s.i(Const(A, 4));
    s.i(StoreSlot(LEN, A));
    update(s, CP, IntOp::And, 0x07);
    s.i(Mark(rest));
    s.i(Const(A, 1));
    s.i(StoreSlot(K, A));
    s.i(Mark(more));
    s.i(LoadSlot(A, K));
    s.i(LoadSlot(B, LEN));
    s.jump_if(Ge, done);
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, K));
    s.i(Int(IntOp::Add));
    byte_at(s);
    s.i(Const(B, 0x3F));
    s.i(Int(IntOp::And));
    s.i(StoreSlot(TMP, A));
    update(s, CP, IntOp::Mul, 64);
    s.i(LoadSlot(B, TMP));
    s.i(Int(IntOp::Or));
    s.i(StoreSlot(CP, A));
    update(s, K, IntOp::Add, 1);
    s.i(Jump(more));
    s.i(Mark(done));
}

/// `err_shown(text)`: writes the first 40 characters of `text` to standard error, and `…`
/// when there are more. Characters are counted, not bytes: the text is valid UTF-8 and each
/// sequence is decoded whole, so a cut never splits one.
/// Does not allocate.
fn err_shown() -> Function {
    let mut s = Asm::default();
    let [again, cut, done] = [(); 3].map(|_| s.label());
    s.i(LoadArg(A, 1));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(StoreSlot(END, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(AT, A));
    s.i(StoreSlot(COUNT, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, END));
    s.jump_if(Ge, done);
    s.i(LoadSlot(A, COUNT));
    s.i(Const(B, SHOWN_CHARS));
    s.jump_if(Ge, cut);
    decode(&mut s);
    s.push_const(2);
    s.i(LoadArg(A, 1));
    s.i(LoadSlot(B, AT));
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.i(LoadSlot(A, LEN));
    s.i(Push(A));
    s.call("write_all");
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, LEN));
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(AT, A));
    update(&mut s, COUNT, IntOp::Add, 1);
    s.i(Jump(again));
    s.i(Mark(cut));
    s.push_text(super::CUT);
    s.call("err_text");
    s.i(Mark(done));
    s.finish("err_shown", 1, 7)
}

pub(super) fn routines() -> Vec<Function> {
    vec![err_shown()]
}

/// The characters `chars::unshowable` rejects, as sorted inclusive ranges: for each, the
/// first and last code point as 8-byte little-endian integers. It is generated from
/// `unshowable` itself, so the runtime and the compile-error echoes never disagree. A
/// runtime text holds it, so the routine reads entry `i` at byte `16 * i` of the text.
pub(super) fn unshowable_table() -> Vec<u8> {
    let mut table = Vec::new();
    let mut open: Option<(u32, u32)> = None;
    let close = |range: (u32, u32), table: &mut Vec<u8>| {
        table.extend(u64::from(range.0).to_le_bytes());
        table.extend(u64::from(range.1).to_le_bytes());
    };
    for code in 0..=char::MAX as u32 {
        let Some(c) = char::from_u32(code).filter(|c| unshowable(*c)) else {
            continue;
        };
        open = match open {
            Some((first, last)) if last + 1 == c as u32 => Some((first, c as u32)),
            Some(range) => {
                close(range, &mut table);
                Some((c as u32, c as u32))
            }
            None => Some((c as u32, c as u32)),
        };
    }
    if let Some(range) = open {
        close(range, &mut table);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run, xorshift};
    use super::*;
    use crate::ir::Data;

    fn shown(text: &str) -> String {
        let mut out: String = text
            .chars()
            .take(40)
            .map(|c| match unshowable(c) {
                true => format!("\\u{{{:04X}}}", c as u32),
                false => c.to_string(),
            })
            .collect();
        if text.chars().count() > 40 {
            out.push('\u{2026}');
        }
        out
    }

    #[test]
    fn err_shown_cuts_at_40_characters_whole_sequences() {
        let pool = ['a', 'z', 'é', '\u{2026}', '\u{1F600}', ' '];
        let mut texts: Vec<String> = [0, 1, 39, 40, 41, 42]
            .iter()
            .map(|n| "a".repeat(*n))
            .collect();
        texts.push(format!("{}é\u{2026}", "a".repeat(39)));
        texts.push(format!("{}\u{1F600}\u{1F600}", "é".repeat(39)));
        let mut state = 0x2545_F491_4F6C_DD1D;
        while texts.len() < 300 {
            let length = 36 + xorshift(&mut state) % 8;
            texts.push(
                (0..length)
                    .map(|_| pool[(xorshift(&mut state) % 6) as usize])
                    .collect(),
            );
        }
        let mut raw: Vec<&[u8]> = vec![b"\n"];
        raw.extend(texts.iter().map(|t| t.as_bytes()));
        let mut body = Vec::new();
        for i in 0..texts.len() {
            body.extend([
                AddressOf(A, Data::Text(i as u32 + 1)),
                Push(A),
                Call("err_shown".into()),
            ]);
            body.extend([
                AddressOf(A, Data::Text(0)),
                Push(A),
                Call("err_text".into()),
            ]);
        }
        let o = run(&program(body, &raw));
        let want: String = texts.iter().map(|t| format!("{}\n", shown(t))).collect();
        assert_eq!(o.status, 0);
        assert_eq!(String::from_utf8(o.stderr).unwrap(), want);
    }

    #[test]
    fn the_table_holds_exactly_the_unshowable_characters() {
        let table = unshowable_table();
        let number = |at: usize| u64::from_le_bytes(table[at..at + 8].try_into().unwrap());
        let ranges: Vec<(u64, u64)> = (0..table.len() / 16)
            .map(|i| (number(16 * i), number(16 * i + 8)))
            .collect();
        assert_eq!(table.len() % 16, 0);
        assert!(ranges.windows(2).all(|w| w[0].1 + 1 < w[1].0));
        for code in 0..=char::MAX as u32 {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            let listed = ranges
                .iter()
                .any(|r| (r.0..=r.1).contains(&u64::from(code)));
            assert_eq!(listed, unshowable(c), "U+{code:04X}");
        }
    }
}
