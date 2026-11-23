use std::collections::HashMap;

use super::{CODE_BASE, DATA_BASE, Image, RODATA_BASE, write};

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
        symbols: HashMap::new(),
        data_size: 0,
        globals: HashMap::new(),
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
