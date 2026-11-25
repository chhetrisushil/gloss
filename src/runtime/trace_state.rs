//! The layout of `Global::TraceState`. Each constant is a byte offset; a word is 8 bytes.
//! The state is zero at the start. `ESCAPING`: text entering the output buffer is
//! JSON-escaped. `FAILING`: writes to descriptor 2 are the failure message. `OPEN`: a `show`
//! event is open, its text being written.

use super::asm::Asm;
use crate::ir::{Data, Global, Inst::*, Reg};

pub const ESCAPING: i32 = 0;
// `FAILING` and `OPEN` are first used by the failure and `show` events (later tasks).
#[allow(dead_code)]
pub const FAILING: i32 = 8;
#[allow(dead_code)]
pub const OPEN: i32 = 16;

/// Loads the word at byte `offset` of the state into `to`.
pub(super) fn get(s: &mut Asm, to: Reg, offset: i32) {
    s.i(AddressOf(to, Data::Global(Global::TraceState)));
    s.i(Load {
        to,
        base: to,
        offset,
    });
}

/// Stores `from` into the word at byte `offset` of the state, using `tmp` for the address.
#[allow(dead_code)]
pub(super) fn set(s: &mut Asm, offset: i32, from: Reg, tmp: Reg) {
    s.i(AddressOf(tmp, Data::Global(Global::TraceState)));
    s.i(Store {
        base: tmp,
        offset,
        from,
    });
}
