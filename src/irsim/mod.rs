//! `irsim`: a test-only interpreter for the Gloss IR. It follows the IR's documented
//! semantics exactly, so runtime routines written in IR can be checked against the Rust
//! `Number` reference and whole programs can run on any host.

mod cases;
mod exec;
mod machine;
pub use machine::INITIAL_STACK_TOP;
mod memory;
mod num;
mod system;
#[cfg(test)]
mod tests;

use crate::ir::Module;
use machine::{Ended, Machine};

/// What a whole program did.
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
    /// How many `Write` calls the program made.
    pub writes: u32,
}

/// Faults a test can make `Write` show, as a real kernel sometimes does.
#[derive(Default)]
pub struct Faults {
    /// The initial stack's auxiliary vector has no `AT_EXECFN` entry.
    pub no_exec_name: bool,
    /// `AT_EXECFN` comes after 64 other pairs, past the walk's bound.
    pub far_exec_name: bool,
    /// The executable's name has 5000 bytes before its NUL, past the walk's bound.
    pub long_exec_name: bool,
    /// `Write` takes at most this many bytes per call (a short write).
    pub short_write: Option<u64>,
    /// `Write` to these descriptors fails with a negative result.
    pub failing_fds: Vec<i64>,
}

/// Runs the function `entry` as a program, feeding it `stdin`.
pub fn run(m: &Module, entry: &str, stdin: &[u8]) -> Outcome {
    run_with(m, entry, stdin, Faults::default())
}

/// Like `run`, with write faults.
pub fn run_with(m: &Module, entry: &str, stdin: &[u8], faults: Faults) -> Outcome {
    let mut machine = Machine::new(m);
    machine.faults = faults;
    machine.stdin = stdin.to_vec();
    let status = match machine.execute(entry, &[]) {
        Ended::Returned(_) => 0,
        Ended::Exited(status) => status,
    };
    Outcome {
        stdout: machine.stdout,
        stderr: machine.stderr,
        status,
        writes: machine.writes,
    }
}

/// Runs the one function `name` with `args`, for unit tests.
pub fn call(m: &Module, name: &str, args: &[i64]) -> i64 {
    match Machine::new(m).execute(name, args) {
        Ended::Returned(result) => result,
        Ended::Exited(status) => panic!("irsim: {name} exited with status {status}"),
    }
}
