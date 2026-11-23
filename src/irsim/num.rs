//! The `Num*` instructions: each means exactly the matching `Number` method.

use super::machine::Machine;
use crate::ir::Label;
use crate::number::{NumFail, Number};

impl Machine<'_> {
    /// Finishes a `Num*` instruction: puts a success in `a`, or jumps to the label the
    /// failure names. Instructions that cannot divide by zero pass `range` for `zero` too.
    pub fn settle(&mut self, result: Result<Number, NumFail>, zero: Label, range: Label) {
        match result {
            Ok(Number(n)) => self.a = n,
            Err(NumFail::DivideByZero) => self.jump(zero),
            Err(NumFail::OutOfRange) => self.jump(range),
        }
    }
}
