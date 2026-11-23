//! The simulated address space: sparse little-endian bytes, with the read-only texts,
//! descriptors and globals laid out at fixed addresses. Unwritten bytes read as zero.

use crate::ir::{Data, Global, Module};
use std::collections::HashMap;

/// Where read-only texts start: each is an 8-byte little-endian length, then the bytes,
/// padded to 8-byte alignment.
const TEXT_BASE: u64 = 0x1000_0000;
/// Where `Prim::Map` hands out memory.
const MAP_BASE: u64 = 0x4000_0000_0000;
/// The most one `Prim::Map` call may ask for.
const MAP_LIMIT: i64 = 1 << 40;
/// Where the globals start, in `Global` declaration order.
const GLOBAL_BASE: u64 = 0x2000_0000;

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

fn global_size(g: Global) -> u64 {
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

fn global_address(g: Global) -> u64 {
    let before = GLOBALS.iter().take_while(|&&other| other != g);
    GLOBAL_BASE + before.map(|&other| global_size(other)).sum::<u64>()
}

pub struct Memory {
    bytes: HashMap<u64, u8>,
    text_addrs: Vec<u64>,
    desc_addrs: Vec<u64>,
    /// The end of the read-only texts and descriptors.
    rodata_end: u64,
    /// The blocks `Prim::Map` handed out, as start and end addresses.
    mapped: Vec<(u64, u64)>,
}

impl Memory {
    /// An address space holding `module`'s texts and descriptors.
    pub fn new(module: &Module) -> Memory {
        let mut memory = Memory {
            bytes: HashMap::new(),
            text_addrs: Vec::new(),
            desc_addrs: Vec::new(),
            rodata_end: TEXT_BASE,
            mapped: Vec::new(),
        };
        let mut next = TEXT_BASE;
        for text in &module.texts {
            memory.text_addrs.push(next);
            memory.write(next, text.len() as i64);
            for (i, byte) in text.iter().enumerate() {
                memory.write_byte(next + 8 + i as u64, *byte);
            }
            next += (8 + text.len() as u64).next_multiple_of(8);
        }
        // Task 9 defines the descriptor layout; until then each descriptor is an 8-byte
        // placeholder, so `AddressOf(Desc(_))` still gives distinct addresses.
        for _ in &module.descs {
            memory.desc_addrs.push(next);
            next += 8;
        }
        memory.rodata_end = next;
        memory
    }

    pub fn address_of(&self, data: Data) -> u64 {
        match data {
            Data::Text(i) => self.text_addrs[i as usize],
            Data::Desc(i) => self.desc_addrs[i as usize],
            Data::Global(g) => global_address(g),
        }
    }

    /// Hands out `len` fresh zeroed bytes, or gives 0 when `len` is negative or above 1 TiB.
    /// Unwritten bytes read as zero, so the block needs no filling.
    pub fn map(&mut self, len: i64) -> u64 {
        if !(0..=MAP_LIMIT).contains(&len) {
            return 0;
        }
        let start = self.mapped.last().map_or(MAP_BASE, |&(_, end)| end);
        self.mapped
            .push((start, start + (len as u64).next_multiple_of(8)));
        start
    }

    /// Whether the `len` bytes at `addr` lie in the read-only data, or (when `write` is set,
    /// only there) in the globals or a mapped block. The stack is the machine's to check.
    pub fn allows(&self, addr: u64, len: u64, write: bool) -> bool {
        let Some(end) = addr.checked_add(len) else {
            return false;
        };
        let globals_end = GLOBAL_BASE + GLOBALS.iter().map(|&g| global_size(g)).sum::<u64>();
        let in_rodata = addr >= TEXT_BASE && end <= self.rodata_end;
        let in_globals = addr >= GLOBAL_BASE && end <= globals_end;
        let in_mapped = self.mapped.iter().any(|&(s, e)| addr >= s && end <= e);
        in_globals || in_mapped || (in_rodata && !write)
    }

    pub fn read_byte(&self, addr: u64) -> u8 {
        self.bytes.get(&addr).copied().unwrap_or(0)
    }

    pub fn write_byte(&mut self, addr: u64, value: u8) {
        self.bytes.insert(addr, value);
    }

    pub fn read(&self, addr: u64) -> i64 {
        let mut bytes = [0u8; 8];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.read_byte(addr.wrapping_add(i as u64));
        }
        i64::from_le_bytes(bytes)
    }

    pub fn write(&mut self, addr: u64, value: i64) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write_byte(addr.wrapping_add(i as u64), byte);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_little_endian_and_unwritten_bytes_are_zero() {
        let mut memory = Memory::new(&Module::default());
        assert_eq!(memory.read(0x5000), 0);
        memory.write(0x5000, 0x0102_0304_0506_0708);
        assert_eq!(memory.read_byte(0x5000), 0x08);
        assert_eq!(memory.read_byte(0x5007), 0x01);
        assert_eq!(memory.read(0x5000), 0x0102_0304_0506_0708);
    }

    #[test]
    fn texts_are_a_length_then_bytes_each_8_byte_aligned() {
        let mut m = Module::default();
        let first = m.text(b"hello");
        let second = m.text(b"");
        let memory = Memory::new(&m);
        let at = memory.address_of(Data::Text(first));
        assert_eq!(at, 0x1000_0000);
        assert_eq!(memory.read(at), 5);
        assert_eq!(memory.read_byte(at + 8), b'h');
        assert_eq!(memory.read_byte(at + 12), b'o');
        assert_eq!(memory.address_of(Data::Text(second)), 0x1000_0010);
    }

    #[test]
    fn descriptors_follow_the_texts() {
        let mut m = Module::default();
        m.text(b"abc");
        let number = m.desc(crate::ir::Desc::Number);
        let text = m.desc(crate::ir::Desc::Text);
        let memory = Memory::new(&m);
        assert_eq!(memory.address_of(Data::Desc(number)), 0x1000_0010);
        assert_eq!(memory.address_of(Data::Desc(text)), 0x1000_0018);
    }

    #[test]
    fn globals_follow_declaration_order() {
        let memory = Memory::new(&Module::default());
        let at = |g| memory.address_of(Data::Global(g));
        assert_eq!(at(Global::HeapNext), 0x2000_0000);
        assert_eq!(at(Global::StackLimit), 0x2000_0010);
        assert_eq!(at(Global::OutBuf), 0x2000_0020);
        assert_eq!(at(Global::InLen), 0x2000_0020 + 4096);
        assert_eq!(at(Global::InBuf), 0x2000_0020 + 4096 + 16);
    }
}
