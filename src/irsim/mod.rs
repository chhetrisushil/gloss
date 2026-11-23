//! `irsim`: a test-only interpreter for the Gloss IR. It follows the IR's documented
//! semantics exactly, so runtime routines written in IR can be checked against the Rust
//! `Number` reference and whole programs can run on any host.

mod exec;
mod machine;
mod memory;
mod num;
#[cfg(test)]
mod tests;

use crate::ir::Module;
use machine::Machine;

/// What a whole program did.
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// Runs the function `entry` as a program, feeding it `stdin`.
pub fn run(m: &Module, entry: &str, stdin: &[u8]) -> Outcome {
    let _ = stdin; // Task 3b's `Prim::Read` consumes it.
    Machine::new(m).execute(entry, &[]);
    Outcome {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: 0,
    }
}

/// Runs the one function `name` with `args`, for unit tests.
pub fn call(m: &Module, name: &str, args: &[i64]) -> i64 {
    Machine::new(m).execute(name, args)
}
