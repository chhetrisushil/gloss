//! The runtime, written in IR: output and failure messages, memory and the stack limit.
//! Programs call these routines by name.
//!
//! The runtime's fixed texts are `TEXTS`; a routine names one as `Data::Text(i)` with `i` an
//! index into it, and `add_reachable` moves the ones it needs into the module. So
//! `routines()` is only meaningful as input to `add_reachable`.

mod asm;
mod core;
mod numbers;
mod output;
mod parse;
mod places;
#[cfg(test)]
mod tests;
mod text;

use crate::ir::{Data, Function, Inst, Module};

const LINE: u32 = 0;
const FAILURE: u32 = 1;
const COLON: u32 = 2;
const NEWLINE: u32 = 3;
const OUT_OF_MEMORY: u32 = 4;

/// The texts the routines refer to.
const TEXTS: [&[u8]; 5] = [b"line ", b": failure ", b": ", b"\n", b"out of memory"];

/// Every runtime routine.
pub fn routines() -> Vec<Function> {
    let mut all = core::routines();
    all.extend(output::routines());
    all.extend(numbers::routines());
    all.extend(parse::routines());
    all.extend(places::routines());
    all.extend(text::routines());
    all
}

/// Adds to `m` the routines named in `called`, and those they call, once each. Names that
/// are not runtime routines are ignored.
pub fn add_reachable(m: &mut Module, called: &[String]) {
    let all = routines();
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
                Inst::AddressOf(_, Data::Text(i)) => *i = m.text(TEXTS[*i as usize]),
                _ => {}
            }
        }
        m.functions.push(function);
    }
}
