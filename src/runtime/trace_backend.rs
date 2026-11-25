//! The routines a trace build swaps in for the real ones. None yet: later tasks add the
//! event writers and the replaced output and input routines.

use crate::ir::Function;

/// Names of the real routines a trace build replaces.
pub const REPLACED: [&str; 0] = [];

/// The routines a trace build adds.
pub fn routines() -> Vec<Function> {
    Vec::new()
}
