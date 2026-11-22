//! `irsim`: a test-only interpreter for the Gloss IR. It follows the IR's documented
//! semantics exactly, so runtime routines written in IR can be checked against the Rust
//! `Number` reference and whole programs can run on any host.

mod exec;
mod machine;
mod memory;
#[cfg(test)]
mod tests;

use crate::ir::Module;
use machine::Machine;

/// Runs the one function `name` with `args`, for unit tests.
pub fn call(m: &Module, name: &str, args: &[i64]) -> i64 {
    Machine::new(m).execute(name, args)
}
