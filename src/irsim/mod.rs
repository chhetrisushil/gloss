//! `irsim`: a test-only interpreter for the Gloss IR. It follows the IR's documented
//! semantics exactly, so runtime routines written in IR can be checked against the Rust
//! `Number` reference and whole programs can run on any host.

mod cases;
mod exec;
mod files;
pub use files::Files;
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
    /// The files when the program ended.
    pub files: Files,
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
    /// `Read` fails with a negative result.
    pub failing_read: bool,
    /// `Open` of this normalised path fails with a permission error.
    pub failing_open: Option<String>,
    /// `Write` to a descriptor from `Open` fails with an I/O error.
    pub failing_file_write: bool,
}

/// Runs the function `entry` as a program, feeding it `stdin`.
pub fn run(m: &Module, entry: &str, stdin: &[u8]) -> Outcome {
    let (faults, mut files) = (Faults::default(), Files::default());
    run_with(m, entry, stdin, &[], faults, &mut files)
}

/// Like `run`, with the program arguments (after `argv[0]`), faults and the starting
/// `files`, which hold the final state afterwards, too.
pub fn run_with(
    m: &Module,
    entry: &str,
    stdin: &[u8],
    args: &[&str],
    faults: Faults,
    files: &mut Files,
) -> Outcome {
    let mut machine = Machine::new(m);
    machine.files = files.clone();
    machine.faults = faults;
    machine.stdin = stdin.to_vec();
    machine.args = args.iter().map(|a| a.to_string()).collect();
    let status = match machine.execute(entry, &[]) {
        Ended::Returned(_) => 0,
        Ended::Exited(status) => status,
    };
    *files = machine.files.clone();
    Outcome {
        stdout: machine.stdout,
        stderr: machine.stderr,
        status,
        writes: machine.writes,
        files: machine.files,
    }
}

/// Runs the one function `name` with `args`, for unit tests.
pub fn call(m: &Module, name: &str, args: &[i64]) -> i64 {
    match Machine::new(m).execute(name, args) {
        Ended::Returned(result) => result,
        Ended::Exited(status) => panic!("irsim: {name} exited with status {status}"),
    }
}
