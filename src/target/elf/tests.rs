use std::collections::HashMap;

use super::{CODE_BASE, DATA_BASE, Image, RODATA_BASE, write};
use crate::ir::Global;
use crate::target::x86_64::encode::Fixup;
use crate::target::x86_64::inst::Sym;

fn le(b: &[u8], at: usize, width: usize) -> u64 {
    b[at..at + width]
        .iter()
        .rev()
        .fold(0, |n, byte| n << 8 | u64::from(*byte))
}

fn image(code: Vec<u8>) -> Image {
    Image {
        code,
        entry: 2,
        fixups: vec![],
        rodata: vec![1, 2, 3],
        symbols: HashMap::from([(Sym::Text(0), 1)]),
        data_size: 0,
        globals: HashMap::from([(Global::OutBuf, 16)]),
    }
}

fn phdr(b: &[u8], n: usize) -> [u64; 7] {
    let o = 64 + 56 * n;
    [(0, 4), (4, 4), (8, 8), (16, 8), (32, 8), (40, 8), (48, 8)].map(|(k, w)| le(b, o + k, w))
}

#[test]
fn the_headers_carry_the_binding_layout() {
    let b = write(&image(vec![0x90; 10])).unwrap();
    assert_eq!(&b[..8], &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
    assert_eq!([le(&b, 16, 2), le(&b, 18, 2), le(&b, 20, 4)], [2, 62, 1]);
    assert_eq!([24, 32, 40].map(|n| le(&b, n, 8)), [0x40_0122, 64, 0]);
    assert_eq!([52, 54, 56, 60].map(|n| le(&b, n, 2)), [64, 56, 4, 0]);
    assert_eq!(phdr(&b, 0), [1, 5, 0, CODE_BASE, 298, 298, 0x1000]);
    assert_eq!(phdr(&b, 1), [1, 4, 0x1000, RODATA_BASE, 3, 3, 0x1000]);
    assert_eq!(phdr(&b, 2), [1, 6, 0x2000, DATA_BASE, 0, 8, 0x1000]);
    assert_eq!(phdr(&b, 3), [0x6474_e551, 6, 0, 0, 0, 0, 0x1000]);
    assert_eq!(&b[0x1000..0x1003], &[1, 2, 3]);
}

#[test]
fn every_load_segment_is_congruent_and_inside_the_file() {
    for len in [0, 1, 4095, 4096, 4097] {
        let mut img = image(vec![0x90; 10]);
        img.rodata = vec![7; len];
        let b = write(&img).unwrap();
        for n in 0..3 {
            let [_, _, off, vaddr, filesz, _, align] = phdr(&b, n);
            assert_eq!(off % align, vaddr % align, "segment {n}, rodata {len}");
            assert!(off + filesz <= b.len() as u64, "segment {n}, rodata {len}");
        }
        assert!(phdr(&b, 2)[2] >= phdr(&b, 1)[2] + len as u64);
    }
}

fn fix(target: Sym) -> Fixup {
    Fixup {
        at: 0,
        target,
        end: 4,
    }
}

fn with(fixups: Vec<Fixup>) -> Image {
    Image {
        fixups,
        ..image(vec![0; 10])
    }
}

#[test]
fn a_rodata_fixup_is_patched_relative_to_the_instruction_end() {
    let b = write(&with(vec![fix(Sym::Text(0))])).unwrap();
    let want = (RODATA_BASE + 1) as i64 - (CODE_BASE + 288 + 4) as i64;
    assert_eq!(le(&b, 288, 4) as u32 as i32 as i64, want);
}

#[test]
fn a_global_fixup_points_into_the_data_segment() {
    let b = write(&with(vec![fix(Sym::Global(Global::OutBuf))])).unwrap();
    let want = (DATA_BASE + 16) as i64 - (CODE_BASE + 288 + 4) as i64;
    assert_eq!(le(&b, 288, 4) as u32 as i32 as i64, want);
}

#[test]
fn unresolvable_fixups_are_errors() {
    for t in [
        Sym::Label("x".to_string()),
        Sym::Desc(9),
        Sym::Global(Global::InLen),
    ] {
        let e = write(&with(vec![fix(t.clone())])).unwrap_err();
        assert!(e.contains(&format!("{t:?}")), "{e}");
    }
}

#[test]
fn a_displacement_beyond_rel32_is_an_error() {
    let mut img = with(vec![fix(Sym::Text(0))]);
    img.symbols.insert(Sym::Text(0), 0x8000_0000);
    assert!(write(&img).unwrap_err().contains("rel32"));
}

#[test]
fn malformed_fixup_ranges_are_errors_not_panics() {
    let ranges = [
        (0, 11),
        (8, 10),
        (7, 10),
        (3, 4),
        (usize::MAX - 1, usize::MAX),
        (0, 2),
    ];
    for (at, end) in ranges {
        let f = Fixup {
            at,
            target: Sym::Text(0),
            end,
        };
        assert!(
            write(&with(vec![f]))
                .unwrap_err()
                .contains("outside the code")
        );
    }
}

#[test]
fn an_entry_outside_the_code_is_an_error() {
    for entry in [10, 11] {
        let img = Image {
            entry,
            ..image(vec![0; 10])
        };
        assert!(write(&img).unwrap_err().contains("entry"));
    }
}

#[test]
fn segments_that_would_overlap_are_errors() {
    let big_code = Image {
        code: vec![0; 0x1000_0000],
        ..image(vec![])
    };
    assert!(write(&big_code).unwrap_err().contains("overlap"));
    let big_rodata = Image {
        rodata: vec![0; 0x1000_0001],
        ..image(vec![0; 10])
    };
    assert!(write(&big_rodata).unwrap_err().contains("overlap"));
}

#[test]
fn a_data_size_sets_the_memory_size_of_the_data_segment() {
    for (size, memsz) in [(0, 8), (100, 100)] {
        let img = Image {
            data_size: size,
            ..image(vec![0; 10])
        };
        assert_eq!(phdr(&write(&img).unwrap(), 2)[5], memsz);
    }
}
