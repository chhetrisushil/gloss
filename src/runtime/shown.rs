//! Writing a text into a failure message, cut and with invisible characters spelled out.

use crate::chars::unshowable;

/// The characters `chars::unshowable` rejects, as sorted inclusive ranges: for each, the
/// first and last code point as 8-byte little-endian integers. It is generated from
/// `unshowable` itself, so the runtime and the compile-error echoes never disagree. A
/// runtime text holds it, so the routine reads entry `i` at byte `16 * i` of the text.
pub(super) fn unshowable_table() -> Vec<u8> {
    let mut table = Vec::new();
    let mut open: Option<(u32, u32)> = None;
    let close = |range: (u32, u32), table: &mut Vec<u8>| {
        table.extend(u64::from(range.0).to_le_bytes());
        table.extend(u64::from(range.1).to_le_bytes());
    };
    for code in 0..=char::MAX as u32 {
        let Some(c) = char::from_u32(code).filter(|c| unshowable(*c)) else {
            continue;
        };
        open = match open {
            Some((first, last)) if last + 1 == c as u32 => Some((first, c as u32)),
            Some(range) => {
                close(range, &mut table);
                Some((c as u32, c as u32))
            }
            None => Some((c as u32, c as u32)),
        };
    }
    if let Some(range) = open {
        close(range, &mut table);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_exactly_the_unshowable_characters() {
        let table = unshowable_table();
        let number = |at: usize| u64::from_le_bytes(table[at..at + 8].try_into().unwrap());
        let ranges: Vec<(u64, u64)> = (0..table.len() / 16)
            .map(|i| (number(16 * i), number(16 * i + 8)))
            .collect();
        assert_eq!(table.len() % 16, 0);
        assert!(ranges.windows(2).all(|w| w[0].1 + 1 < w[1].0));
        for code in 0..=char::MAX as u32 {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            let listed = ranges
                .iter()
                .any(|r| (r.0..=r.1).contains(&u64::from(code)));
            assert_eq!(listed, unshowable(c), "U+{code:04X}");
        }
    }
}
