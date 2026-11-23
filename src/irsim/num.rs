//! The `Num*` instructions: each means exactly the matching `Number` method.

use super::machine::Machine;
use crate::ir::Label;
use crate::number::{NumFail, Number};

/// What `a` holds after a failure jump: the IR leaves it undefined (M2-R8).
const POISON: i64 = 0x5EAD_5EAD_5EAD_5EAD;

impl Machine<'_> {
    /// Finishes a `Num*` instruction: puts a success in `a`, or poisons `a` and jumps to the
    /// label the failure names. Instructions that cannot divide by zero pass `range` for `zero` too.
    pub fn settle(&mut self, result: Result<Number, NumFail>, zero: Label, range: Label) {
        match result {
            Ok(Number(n)) => self.a = n,
            Err(NumFail::DivideByZero) => {
                self.a = POISON;
                self.jump(zero);
            }
            Err(NumFail::OutOfRange) => {
                self.a = POISON;
                self.jump(range);
            }
        }
    }
}
