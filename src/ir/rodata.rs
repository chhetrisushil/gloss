//! The read-only data both `irsim` and the real executable lay out from a module, so they
//! agree byte for byte: the module's texts, the names the descriptors use, then the
//! descriptors (see `Desc` for their layout).

use std::collections::HashMap;

use super::{Desc, Module};

/// The read-only bytes and where each text and descriptor starts in them.
pub struct Rodata {
    pub bytes: Vec<u8>,
    /// Offsets of `Module::texts`, by index.
    pub texts: Vec<usize>,
    /// Offsets of `Module::descs`, by index.
    pub descs: Vec<usize>,
}

fn push_word(bytes: &mut Vec<u8>, word: u64) {
    bytes.extend_from_slice(&word.to_le_bytes());
}

/// The size in bytes of a descriptor.
fn desc_size(d: &Desc) -> usize {
    match d {
        Desc::Number | Desc::Text | Desc::Truth => 8,
        Desc::Record { fields, .. } => 8 * (3 + 2 * fields.len()),
        Desc::List(_) => 16,
    }
}

/// Lays out `m`'s read-only data to be loaded at address `base`.
pub fn rodata(m: &Module, base: u64) -> Rodata {
    let mut bytes = Vec::new();
    let mut at: HashMap<Vec<u8>, usize> = HashMap::new();
    let add_text = |bytes: &mut Vec<u8>, text: &[u8]| -> usize {
        let start = bytes.len();
        push_word(bytes, text.len() as u64);
        bytes.extend_from_slice(text);
        bytes.resize(bytes.len().next_multiple_of(8), 0);
        start
    };
    let mut texts = Vec::new();
    for text in &m.texts {
        let start = add_text(&mut bytes, text);
        at.entry(text.clone()).or_insert(start);
        texts.push(start);
    }
    for d in &m.descs {
        if let Desc::Record { name, fields } = d {
            for n in std::iter::once(name).chain(fields.iter().map(|(n, _)| n)) {
                if !at.contains_key(n.as_bytes()) {
                    let start = add_text(&mut bytes, n.as_bytes());
                    at.insert(n.as_bytes().to_vec(), start);
                }
            }
        }
    }
    let mut descs = Vec::new();
    let mut next = bytes.len();
    for d in &m.descs {
        descs.push(next);
        next += desc_size(d);
    }
    let address = |offset: usize| base + offset as u64;
    for d in &m.descs {
        match d {
            Desc::Number => push_word(&mut bytes, 0),
            Desc::Text => push_word(&mut bytes, 1),
            Desc::Truth => push_word(&mut bytes, 2),
            Desc::Record { name, fields } => {
                push_word(&mut bytes, 3);
                push_word(&mut bytes, address(at[name.as_bytes()]));
                push_word(&mut bytes, fields.len() as u64);
                for (field, desc) in fields {
                    push_word(&mut bytes, address(at[field.as_bytes()]));
                    push_word(&mut bytes, address(descs[*desc as usize]));
                }
            }
            Desc::List(item) => {
                push_word(&mut bytes, 4);
                push_word(&mut bytes, address(descs[*item as usize]));
            }
        }
    }
    Rodata {
        bytes,
        texts,
        descs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(r: &Rodata, offset: usize) -> u64 {
        u64::from_le_bytes(r.bytes[offset..offset + 8].try_into().unwrap())
    }

    #[test]
    fn a_record_descriptor_names_its_texts_and_field_descriptors() {
        let mut m = Module::default();
        m.text(b"amount");
        let number = m.desc(Desc::Number);
        let text = m.desc(Desc::Text);
        let record = m.desc(Desc::Record {
            name: "E".into(),
            fields: vec![("name".into(), text), ("amount".into(), number)],
        });
        let list = m.desc(Desc::List(record));
        let r = rodata(&m, 0x1000);
        let at = |offset: usize| 0x1000 + offset as u64;
        // `amount` is reused; `E` and `name` follow it.
        assert_eq!(r.texts, vec![0]);
        let (e, name) = (16, 32);
        assert_eq!(word(&r, e), 1);
        assert_eq!(&r.bytes[e + 8..e + 9], b"E");
        assert_eq!(&r.bytes[name + 8..name + 12], b"name");
        let d = r.descs[record as usize];
        assert_eq!(word(&r, d), 3);
        assert_eq!(word(&r, d + 8), at(e));
        assert_eq!(word(&r, d + 16), 2);
        assert_eq!(word(&r, d + 24), at(name));
        assert_eq!(word(&r, d + 32), at(r.descs[text as usize]));
        assert_eq!(word(&r, d + 40), at(0));
        assert_eq!(word(&r, d + 48), at(r.descs[number as usize]));
        let l = r.descs[list as usize];
        assert_eq!((word(&r, l), word(&r, l + 8)), (4, at(d)));
        assert_eq!(r.bytes.len(), l + 16);
    }

    #[test]
    fn a_record_before_its_field_descriptors_still_points_at_them() {
        let m = Module {
            descs: vec![
                Desc::List(1),
                Desc::Record {
                    name: "R".into(),
                    fields: vec![("a".into(), 2), ("b".into(), 3)],
                },
                Desc::Truth,
                Desc::Text,
            ],
            ..Module::default()
        };
        let base = 0x4000;
        let r = rodata(&m, base);
        let at = |i: usize| base + r.descs[i] as u64;
        // The names `R`, `a` and `b` come first, 16 bytes each; then the descriptors in order.
        assert_eq!(r.descs, vec![48, 64, 64 + 56, 64 + 64]);
        let (list, rec) = (r.descs[0], r.descs[1]);
        assert_eq!((word(&r, list), word(&r, list + 8)), (4, at(1)));
        let want = [3, base, 2, base + 16, at(2), base + 32, at(3)];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(word(&r, rec + 8 * i), *w, "word {i}");
        }
        assert_eq!((word(&r, r.descs[2]), word(&r, r.descs[3])), (2, 1));
        assert_eq!(r.bytes.len(), r.descs[3] + 8);
    }
}
