//! Builds a whole executable from an IR module.

use std::collections::HashMap;

use super::encode::encode;
use super::inst::{Sym, X};
use super::lower::lower_function;
use crate::ir::{Global, Module};
use crate::target::elf::{Image, write};

/// The globals in declaration order, as `irsim` lays them out.
const GLOBALS: [Global; 8] = [
    Global::HeapNext,
    Global::HeapEnd,
    Global::StackLimit,
    Global::OutLen,
    Global::OutBuf,
    Global::InLen,
    Global::InPos,
    Global::InBuf,
];

fn global_size(g: Global) -> usize {
    match g {
        Global::OutBuf | Global::InBuf => 4096,
        Global::HeapNext
        | Global::HeapEnd
        | Global::StackLimit
        | Global::OutLen
        | Global::InLen
        | Global::InPos => 8,
    }
}

/// Texts as `irsim` lays them out (an 8-byte little-endian length, the bytes, padding to
/// 8 bytes), then the descriptors.
fn rodata(m: &Module) -> (Vec<u8>, HashMap<Sym, usize>) {
    let mut bytes = Vec::new();
    let mut symbols = HashMap::new();
    for (i, text) in m.texts.iter().enumerate() {
        symbols.insert(Sym::Text(i as u32), bytes.len());
        bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
        bytes.extend_from_slice(text);
        bytes.resize(bytes.len().next_multiple_of(8), 0);
    }
    // Task 9 defines the descriptor layout; until then each is an 8-byte placeholder, as in
    // `irsim`, so `AddressOf(Desc(_))` still gives distinct addresses.
    for i in 0..m.descs.len() {
        symbols.insert(Sym::Desc(i as u32), bytes.len());
        bytes.extend_from_slice(&[0; 8]);
    }
    (bytes, symbols)
}

pub fn executable(m: &Module) -> Result<Vec<u8>, String> {
    let params: HashMap<String, u32> = m
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.params))
        .collect();
    let mut xs: Vec<X> = Vec::new();
    for f in &m.functions {
        xs.extend(lower_function(f, &params)?);
    }
    let code = encode(&xs)?;
    let entry = *code
        .labels
        .get("_start")
        .ok_or("the module has no _start function")?;
    let (rodata, symbols) = rodata(m);
    let mut globals = HashMap::new();
    let mut data_size = 0;
    for g in GLOBALS {
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
