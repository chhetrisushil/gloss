//! A static ELF64 executable for x86-64 Linux: code, read-only data and a zeroed data segment.

use std::collections::HashMap;

use super::x86_64::encode::Fixup;
use super::x86_64::inst::Sym;
use crate::ir::Global;

pub const CODE_BASE: u64 = 0x40_0000;
pub const RODATA_BASE: u64 = 0x1000_0000;
pub const DATA_BASE: u64 = 0x2000_0000;

const PAGE: u64 = 0x1000;
const EHDR_SIZE: usize = 64;
const PHDR_SIZE: usize = 56;
const PHDR_COUNT: usize = 4;
const HEADERS: usize = EHDR_SIZE + PHDR_SIZE * PHDR_COUNT;

const PT_LOAD: u32 = 1;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

/// Everything the writer needs to lay out one executable.
pub struct Image {
    pub code: Vec<u8>,
    pub entry: usize,
    pub fixups: Vec<Fixup>,
    pub rodata: Vec<u8>,
    pub symbols: HashMap<Sym, usize>, // offsets into `rodata`
    pub data_size: usize,
    pub globals: HashMap<Global, usize>, // offsets into the zeroed data segment
}

struct Segment {
    kind: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    filesz: u64,
    memsz: u64,
}

fn align_up(n: u64) -> u64 {
    n.div_ceil(PAGE) * PAGE
}

fn patched_code(img: &Image) -> Result<Vec<u8>, String> {
    match img.fixups.first() {
        Some(f) => Err(format!("fixup to {:?} is not patched yet", f.target)),
        None => Ok(img.code.clone()),
    }
}

/// Appends little-endian fields (value, width in bytes).
fn put(out: &mut Vec<u8>, fields: &[(u64, usize)]) {
    for (v, width) in fields {
        out.extend_from_slice(&v.to_le_bytes()[..*width]);
    }
}

fn put_header(out: &mut Vec<u8>, entry: u64) {
    out.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let (eh, ph, n) = (EHDR_SIZE as u64, PHDR_SIZE as u64, PHDR_COUNT as u64);
    put(
        out,
        &[(2, 2), (62, 2), (1, 4), (entry, 8), (eh, 8), (0, 8), (0, 4)],
    );
    put(out, &[(eh, 2), (ph, 2), (n, 2), (64, 2), (0, 2), (0, 2)]);
}

fn put_segment(out: &mut Vec<u8>, s: &Segment) {
    let (kind, flags) = (u64::from(s.kind), u64::from(s.flags));
    put(
        out,
        &[
            (kind, 4),
            (flags, 4),
            (s.offset, 8),
            (s.vaddr, 8),
            (s.vaddr, 8),
        ],
    );
    put(out, &[(s.filesz, 8), (s.memsz, 8), (PAGE, 8)]);
}

fn load(flags: u32, offset: u64, vaddr: u64, filesz: u64, memsz: u64) -> Segment {
    Segment {
        kind: PT_LOAD,
        flags,
        offset,
        vaddr,
        filesz,
        memsz,
    }
}

/// Lays out the executable. A data size of 0 still gets `p_memsz = 8`, so the layout is
/// uniform. File offsets are page aligned, as the addresses are; the file is padded to
/// the data segment's offset.
pub fn write(img: &Image) -> Result<Vec<u8>, String> {
    if img.entry >= img.code.len() {
        return Err(format!("entry {} lies outside the code", img.entry));
    }
    let code = patched_code(img)?;
    let code_end = (HEADERS + code.len()) as u64;
    let rodata_len = img.rodata.len() as u64;
    let rodata_off = align_up(code_end);
    let data_off = align_up(rodata_off + rodata_len);
    let segments = [
        load(PF_R | PF_X, 0, CODE_BASE, code_end, code_end),
        load(PF_R, rodata_off, RODATA_BASE, rodata_len, rodata_len),
        load(
            PF_R | PF_W,
            data_off,
            DATA_BASE,
            0,
            (img.data_size as u64).max(8),
        ),
        Segment {
            kind: PT_GNU_STACK,
            flags: PF_R | PF_W,
            offset: 0,
            vaddr: 0,
            filesz: 0,
            memsz: 0,
        },
    ];
    let mut out = Vec::new();
    put_header(&mut out, CODE_BASE + HEADERS as u64 + img.entry as u64);
    for s in &segments {
        put_segment(&mut out, s);
    }
    out.extend_from_slice(&code);
    out.resize(rodata_off as usize, 0);
    out.extend_from_slice(&img.rodata);
    out.resize(data_off as usize, 0);
    Ok(out)
}

#[cfg(test)]
mod tests;
