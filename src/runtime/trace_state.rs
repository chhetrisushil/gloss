//! The layout of `Global::TraceState`. Each constant is a byte offset; a word is 8 bytes.
//! The state is zero at the start. `ESCAPING`: text entering the output buffer is
//! JSON-escaped. `FAILING`: writes to descriptor 2 are the failure message. `OPEN`: a `show`
//! event is open, its text being written. `CARRY`: a character of the failure message cut
//! between two writes.

use super::asm::Asm;
use crate::ir::{Data, Global, Inst::*, Reg};

pub const ESCAPING: i32 = 0;
pub const FAILING: i32 = 8;
pub const OPEN: i32 = 16;
/// The incomplete UTF-8 tail of a failure message (see `trace_backend::fail_piece`), at most
/// 3 bytes: bytes 0 to 3 hold its bytes (the fourth only of a 4-byte character as it
/// completes), byte 4 how many there are, byte 5 how long the character is.
pub const CARRY: i32 = 24;

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
pub(super) fn set(s: &mut Asm, offset: i32, from: Reg, tmp: Reg) {
    s.i(AddressOf(tmp, Data::Global(Global::TraceState)));
    s.i(Store {
        base: tmp,
        offset,
        from,
    });
}
