//! Builds a whole executable from an IR module.

use std::collections::HashMap;

use super::arith;
use super::encode::encode;
use super::inst::{Sym, X};
use super::lower::lower_function;
use crate::ir::{self, Global, IO_BUF_BYTES, Module, NUMBER_BUF_BYTES, TEST_STATE_BYTES};
use crate::target::elf::{Image, RODATA_BASE, write};

/// The globals in the order the executable lays them out; `irsim` lays them out the same
/// way (a test in `irsim/memory.rs` checks the order and the sizes).
pub(crate) const GLOBALS: [Global; 12] = [
    Global::HeapNext,
    Global::HeapEnd,
    Global::StackLimit,
    Global::OutLen,
    Global::OutBuf,
    Global::InLen,
    Global::InPos,
    Global::InBuf,
    Global::EntrySp,
    Global::Tables,
    Global::NumBuf,
    Global::TestState,
];

pub(crate) fn global_size(g: Global) -> usize {
    match g {
        Global::NumBuf => NUMBER_BUF_BYTES,
        Global::TestState => TEST_STATE_BYTES,
        Global::OutBuf | Global::InBuf => IO_BUF_BYTES,
        Global::HeapNext
        | Global::HeapEnd
        | Global::StackLimit
        | Global::EntrySp
        | Global::Tables
        | Global::OutLen
        | Global::InLen
        | Global::InPos => 8,
    }
}

/// The read-only data, laid out by `ir::rodata` as `irsim` does.
fn rodata(m: &Module) -> (Vec<u8>, HashMap<Sym, usize>) {
    let laid = ir::rodata(m, RODATA_BASE);
    let texts = laid.texts.iter().enumerate();
    let descs = laid.descs.iter().enumerate();
    let mut symbols = HashMap::new();
    symbols.extend(texts.map(|(i, o)| (Sym::Text(i as u32), *o)));
    symbols.extend(descs.map(|(i, o)| (Sym::Desc(i as u32), *o)));
    (laid.bytes, symbols)
}

pub fn executable(m: &Module) -> Result<Vec<u8>, String> {
    let params: HashMap<String, u32> = m
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.params))
        .collect();
    if let Some(f) = m
        .functions
        .iter()
        .find(|f| f.name.starts_with(arith::PREFIX))
    {
        return Err(format!(
            "function {} uses the reserved {} prefix",
            f.name,
            arith::PREFIX
        ));
    }
    let mut xs: Vec<X> = Vec::new();
    for f in &m.functions {
        xs.extend(lower_function(f, &params)?);
    }
    xs.extend(arith::helpers());
    let code = encode(&xs)?;
    let entry = *code
        .labels
        .get("_start")
        .ok_or("the module has no _start function")?;
    let (rodata, symbols) = rodata(m);
    // `TestState` is the last global, so leaving it out moves no other one. Only a test
    // executable refers to it; a real one stays as M3 laid it out.
    let uses_test_state = m.functions.iter().flat_map(|f| &f.body).any(|i| {
        matches!(
            i,
            ir::Inst::AddressOf(_, ir::Data::Global(Global::TestState))
        )
    });
    let mut globals = HashMap::new();
    let mut data_size = 0;
    for g in GLOBALS {
        if g == Global::TestState && !uses_test_state {
            continue;
        }
        globals.insert(g, data_size);
        data_size += global_size(g);
    }
    write(&Image {
        code: code.bytes,
        entry,
        fixups: code.fixups,
        rodata,
        symbols,
        data_size,
        globals,
    })
}
