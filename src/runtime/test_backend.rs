//! The routines a test build swaps in for real ones (none yet).

use crate::ir::Function;

/// Names of the real routines a test build replaces.
pub const REPLACED: [&str; 0] = [];

/// The test versions, appended after the real routines that are kept.
pub fn routines() -> Vec<Function> {
    Vec::new()
}
