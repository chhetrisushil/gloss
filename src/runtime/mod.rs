//! The runtime, written in IR: output and failure messages, memory and the stack limit.
//! Programs call these routines by name.
//!
//! The runtime's fixed texts are `TEXTS`; a routine names one as `Data::Text(i)` with `i` an
//! index into it, and `add_reachable` moves the ones it needs into the module. So
//! `routines()` is only meaningful as input to `add_reachable`.

mod args;
mod asm;
mod copy;
mod core;
mod csv_cells;
mod csv_header;
mod csv_read;
mod csv_write;
mod equal;
mod file_read;
#[cfg(test)]
mod heap_tests;
mod input;
#[cfg(test)]
mod input_tests;
mod lists;
mod numbers;
mod output;
mod parse;
mod places;
mod report;
mod save;
mod shown;
mod table_read;
mod test_backend;
mod test_state;
#[cfg(test)]
mod test_state_tests;
#[cfg(test)]
mod tests;
mod text;
mod utf8;
mod values;

use crate::ir::{Data, Function, Inst, Module};

const LINE: u32 = 0;
const FAILURE: u32 = 1;
const COLON: u32 = 2;
const NEWLINE: u32 = 3;
const OUT_OF_MEMORY: u32 = 4;
const INPUT_ENDED: u32 = 5;
const QUOTE: u32 = 6;
const YES: u32 = 7;
const NO: u32 = 8;
const WITH: u32 = 9;
const COMMA_SPACE: u32 = 10;
const SPACE: u32 = 11;
const LIST_OF: u32 = 12;
const EMPTY_LIST: u32 = 13;
const NUMBER: u32 = 14;
const TEXT: u32 = 15;
const TRUTH: u32 = 16;
const OPEN: u32 = 17;
const CLOSE: u32 = 18;
const PERMISSIONS_FLAG: u32 = 19;
const CANNOT_WRITE: u32 = 20;
const ITS_FOLDER: u32 = 21;
const NOT_EXIST: u32 = 22;
const THROUGH_LINK: u32 = 23;
const CANNOT_READ: u32 = 24;
const NOT_FOUND_NOTICE: u32 = 25;
const NOT_UTF8: u32 = 26;
const LINE_N: u32 = 27;
const UNCLOSED: u32 = 28;
const COLUMN: u32 = 29;
const NOT_A_FIELD: u32 = 30;
const TWICE: u32 = 31;
const MISSING: u32 = 32;
const IS_EMPTY: u32 = 33;
const SPACE_QUOTE: u32 = 34;
const NOT_NUMBER: u32 = 35;
const NOT_YES_NO: u32 = 36;
const HAS: u32 = 37;
const CELLS_HEADER: u32 = 38;
const MISPLACED: u32 = 39;
const INDENT: u32 = 40;
const PASS: u32 = 41;
const FAIL: u32 = 42;
const LOST: u32 = 43;
const PASSED: u32 = 44;
const FAILED: u32 = 45;

/// The index of the table of invisible characters, which `shown::unshowable_table` builds.
const UNSHOWABLE: u32 = TEXTS.len() as u32;

/// The texts the routines refer to.
const TEXTS: [&[u8]; 46] = [
    b"line ",
    b": failure ",
    b": ",
    b"\n",
    b"out of memory",
    b"input ended while ask was waiting",
    b"\"",
    b"yes",
    b"no",
    b" with ",
    b", ",
    b" ",
    b"list of ",
    b"empty list of ",
    b"number",
    b"text",
    b"truth",
    b"(",
    b")",
    b"--permissions\0",
    b" cannot be written",
    b" (its folder ",
    b" does not exist)",
    b" is reached through a symbolic link",
    b" cannot be read",
    b" not found; starting with an empty table\n",
    b" is not UTF-8 text (line ",
    b" line ",
    b"a quoted cell is not closed",
    b"column ",
    b" is not a field of ",
    b" appears twice",
    b" is missing",
    b" is empty",
    b" \"",
    b"\" is not a number",
    b"\" is not yes or no",
    b"has ",
    b" cells; the header has ",
    b"a quote is out of place",
    b"        ",
    b"pass  ",
    b"fail  ",
    b"ended without a result (status ",
    b" passed, ",
    b" failed\n",
];

/// The bytes of the runtime text `i`.
fn text_bytes(i: u32) -> Vec<u8> {
    if i == UNSHOWABLE {
        shown::unshowable_table()
    } else {
        TEXTS[i as usize].to_vec()
    }
}

/// Which routines a module is built from: the real ones, or a test build's, where
/// `test_backend` replaces some.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Real,
    Test,
}

/// The routines of `backend`: the real ones, and for `Test` those in `test_backend::REPLACED`
/// swapped for `test_backend::routines()`.
pub fn routines_for(backend: Backend) -> Vec<Function> {
    routines_swapping(backend, &test_backend::REPLACED, test_backend::routines())
}

/// `routines_for` with the replaced names and the replacements given. `Real` ignores both.
fn routines_swapping(backend: Backend, replaced: &[&str], extra: Vec<Function>) -> Vec<Function> {
    let mut all = routines();
    for name in replaced {
        assert!(
            all.iter().any(|f| f.name == *name),
            "`{name}` is replaced but is not a real routine: a typo would swap nothing"
        );
    }
    for f in &extra {
        assert!(
            replaced.contains(&f.name.as_str()) || !all.iter().any(|r| r.name == f.name),
            "test routine `{}` has a real routine's name but does not replace it: \
             add_reachable takes the first of a name and would silently use the real one",
            f.name
        );
    }
    if backend == Backend::Test {
        all.retain(|f| !replaced.contains(&f.name.as_str()));
        all.extend(extra);
    }
    all
}

/// Every real runtime routine.
pub fn routines() -> Vec<Function> {
    let mut all = core::routines();
    all.extend(args::routines());
    all.extend(copy::routines());
    all.extend(csv_cells::routines());
    all.extend(csv_header::routines());
    all.extend(csv_read::routines());
    all.extend(csv_write::routines());
    all.extend(equal::routines());
    all.extend(file_read::routines());
    all.extend(output::routines());
    all.extend(input::routines());
    all.extend(lists::routines());
    all.extend(numbers::routines());
    all.extend(parse::routines());
    all.extend(places::routines());
    all.extend(report::routines());
    all.extend(save::routines());
    all.extend(shown::routines());
    all.extend(table_read::routines());
    all.extend(test_state::routines());
    all.extend(text::routines());
    all.extend(utf8::routines());
    all.extend(values::routines());
    all
}

/// Adds to `m` the routines named in `called`, and those they call, once each. Names that
/// are not runtime routines are ignored.
pub fn add_reachable(m: &mut Module, called: &[String]) {
    add_reachable_for(m, called, Backend::Real);
}

/// `add_reachable`, taking the routines from `backend`.
pub fn add_reachable_for(m: &mut Module, called: &[String], backend: Backend) {
    let all = routines_for(backend);
    let mut pending: Vec<String> = called.to_vec();
    while let Some(name) = pending.pop() {
        if m.functions.iter().any(|f| f.name == name) {
            continue;
        }
        let Some(found) = all.iter().find(|f| f.name == name) else {
            continue;
        };
        let mut function = found.clone();
        for inst in &mut function.body {
            match inst {
                Inst::Call(callee) => pending.push(callee.clone()),
                Inst::AddressOf(_, Data::Text(i)) => *i = m.text(&text_bytes(*i)),
                _ => {}
            }
        }
        m.functions.push(function);
    }
}
