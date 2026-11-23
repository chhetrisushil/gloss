//! The runtime, written in IR: output and failure messages, memory and the stack limit.
//! Programs call these routines by name.

mod asm;
mod output;
#[cfg(test)]
mod tests;

use crate::ir::{Function, Inst, Module};

/// Every runtime routine.
pub fn routines() -> Vec<Function> {
    output::routines()
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
        for inst in &found.body {
            if let Inst::Call(callee) = inst {
                pending.push(callee.clone());
            }
        }
        m.functions.push(found.clone());
    }
}
