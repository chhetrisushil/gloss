//! Writing text into the output buffer as the inside of a JSON string.

use super::asm::Asm;
use super::shown::{AT, CP, END, LEN, TMP, decode};
use super::values::out_const;
use super::{
    JSON_BRACE, JSON_COLON, JSON_COMMA, JSON_FIELDS, JSON_LIST, JSON_LIST_END, JSON_NO,
    JSON_NUMBER, JSON_NUMBER_END, JSON_RECORD, JSON_RECORD_END, JSON_TEXT, JSON_YES, QUOTE,
};
use crate::ir::Cond::{Eq, Ge, Gt, Lt, Ne};
use crate::ir::Reg::{self, A, B};
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

/// `json_int(n)`: the whole number `n` in decimal. It is `number_format` of `n` in
/// ten-thousandths, so it holds for the 15 whole digits `number_format` shows; lines, items,
/// turns, row counts and failure codes are far below that.
fn json_int() -> Function {
    let mut s = Asm::default();
    s.i(LoadArg(A, 1));
    s.i(Const(B, 10_000));
    s.i(Int(IntOp::Mul));
    s.i(Push(A));
    s.call("number_format");
    s.i(Push(A));
    s.call("out_text");
    s.finish("json_int", 1, 0)
}

/// `json_text(text)`: the text as a JSON string, in double quotes.
fn json_text() -> Function {
    let mut s = Asm::default();
    out_const(&mut s, QUOTE);
    s.push_text_arg(1);
    s.call("json_bytes");
    out_const(&mut s, QUOTE);
    s.finish("json_text", 1, 0)
}

// Slots of `json_record` and `json_list`.
const INDEX: u32 = 1;
const COUNT: u32 = 2;

/// Writes `,` before every item but the first, when the index in `INDEX` is not 0.
fn comma_between(s: &mut Asm) {
    let first = s.label();
    s.i(LoadSlot(A, INDEX));
    s.i(Const(B, 0));
    s.jump_if(Eq, first);
    out_const(s, JSON_COMMA);
    s.i(Mark(first));
}

/// `to` = the word at `offset` past the address in `base`.
fn load(s: &mut Asm, to: Reg, base: Reg, offset: i32) {
    s.i(Load { to, base, offset });
}

/// `a` = the word at `offset` past the argument `base` plus `INDEX` times `1 << shift`.
fn indexed(s: &mut Asm, base: u32, shift: u8, offset: i32) {
    s.i(LoadSlot(A, INDEX));
    s.i(Shift(Shift::Left, shift));
    s.i(LoadArg(B, base));
    s.i(Int(IntOp::Add));
    s.i(Load {
        to: A,
        base: A,
        offset,
    });
}

/// `json_record(value, desc)`: `{"record":"E","fields":{"f":v,…}}`, fields in declaration
/// order. A field's name sits at `desc + 24 + 16 * i`, its descriptor 8 bytes on, its value at
/// `value + 8 * i`.
fn json_record() -> Function {
    let mut s = Asm::default();
    let (again, done) = (s.label(), s.label());
    out_const(&mut s, JSON_RECORD);
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 8);
    s.i(Push(A));
    s.call("json_text");
    out_const(&mut s, JSON_FIELDS);
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 16);
    s.i(StoreSlot(COUNT, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(INDEX, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, INDEX));
    s.i(LoadSlot(B, COUNT));
    s.jump_if(Ge, done);
    comma_between(&mut s);
    indexed(&mut s, 2, 4, 24);
    s.i(Push(A));
    s.call("json_text");
    out_const(&mut s, JSON_COLON);
    indexed(&mut s, 1, 3, 0);
    s.i(Push(A));
    indexed(&mut s, 2, 4, 32);
    s.i(Push(A));
    s.call("json_value");
    s.i(LoadSlot(A, INDEX));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(INDEX, A));
    s.i(Jump(again));
    s.i(Mark(done));
    out_const(&mut s, JSON_RECORD_END);
    s.finish("json_record", 2, 2)
}

/// `json_list(list, desc)`: `{"list":[v,…]}`. The list is `[count, capacity, items]`, the item
/// at `items + 8 * i`; the item descriptor is the word after the kind. Slot 3 holds it.
fn json_list() -> Function {
    let mut s = Asm::default();
    let (again, done) = (s.label(), s.label());
    out_const(&mut s, JSON_LIST);
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 8);
    s.i(StoreSlot(3, A));
    s.i(LoadArg(A, 1));
    load(&mut s, A, A, 0);
    s.i(StoreSlot(COUNT, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(INDEX, A));
    s.i(Mark(again));
    s.i(LoadSlot(A, INDEX));
    s.i(LoadSlot(B, COUNT));
    s.jump_if(Ge, done);
    comma_between(&mut s);
    s.i(LoadArg(B, 1));
    load(&mut s, B, B, 16);
    s.i(LoadSlot(A, INDEX));
    s.i(Shift(Shift::Left, 3));
    s.i(Int(IntOp::Add));
    load(&mut s, A, A, 0);
    s.i(Push(A));
    s.push_slot(3);
    s.call("json_value");
    s.i(LoadSlot(A, INDEX));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(INDEX, A));
    s.i(Jump(again));
    s.i(Mark(done));
    out_const(&mut s, JSON_LIST_END);
    s.finish("json_list", 2, 3)
}

/// `json_value(value, desc)`: the typed JSON value by the kind of the descriptor (see
/// `ir::Desc`), as `show_value` walks it.
fn json_value() -> Function {
    let mut s = Asm::default();
    let (text, truth, no, record, list) = (s.label(), s.label(), s.label(), s.label(), s.label());
    s.i(LoadArg(A, 2));
    load(&mut s, A, A, 0);
    s.i(Const(B, 3));
    s.jump_if(Eq, record);
    s.i(Const(B, 4));
    s.jump_if(Eq, list);
    s.i(Const(B, 1));
    s.jump_if(Eq, text);
    s.i(Const(B, 2));
    s.jump_if(Eq, truth);
    out_const(&mut s, JSON_NUMBER);
    s.push_arg(1);
    s.call("out_number");
    out_const(&mut s, JSON_NUMBER_END);
    s.i(Return);
    s.i(Mark(text));
    out_const(&mut s, JSON_TEXT);
    s.push_arg(1);
    s.call("json_text");
    out_const(&mut s, JSON_BRACE);
    s.i(Return);
    s.i(Mark(truth));
    s.i(LoadArg(A, 1));
    s.i(Const(B, 0));
    s.jump_if(Eq, no);
    out_const(&mut s, JSON_YES);
    s.i(Return);
    s.i(Mark(no));
    out_const(&mut s, JSON_NO);
    s.i(Return);
    for (label, routine) in [(record, "json_record"), (list, "json_list")] {
        s.i(Mark(label));
        s.push_arg(1);
        s.push_arg(2);
        s.call(routine);
        s.i(Return);
    }
    s.finish("json_value", 2, 0)
}

pub fn routines() -> Vec<Function> {
    vec![
        json_bytes(),
        json_int(),
        json_text(),
        json_value(),
        json_record(),
        json_list(),
    ]
}
