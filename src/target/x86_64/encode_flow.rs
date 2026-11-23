//! Labels, jumps, calls and the resolution of label fixups.

use super::encode::{Encoded, Fixup};
use super::inst::Sym;

/// `op rel32` to `label`; the displacement is patched by `resolve`.
pub fn branch(e: &mut Encoded, op: &[u8], label: &str) {
    e.bytes.extend_from_slice(op);
    let at = e.bytes.len();
    e.bytes.extend_from_slice(&[0; 4]);
    e.fixups.push(Fixup {
        at,
        target: Sym::Label(label.to_string()),
        end: e.bytes.len(),
    });
}

/// Records `name` at the current offset.
pub fn label(e: &mut Encoded, name: &str) -> Result<(), String> {
    if e.labels.insert(name.to_string(), e.bytes.len()).is_some() {
        return Err(format!("label {name} defined twice"));
    }
    Ok(())
}

/// Patches every fixup that targets a label and drops it from the list.
pub fn resolve(e: &mut Encoded) -> Result<(), String> {
    let mut data = Vec::new();
    for f in std::mem::take(&mut e.fixups) {
        let Sym::Label(name) = &f.target else {
            data.push(f);
            continue;
        };
        let to = *e
            .labels
            .get(name)
            .ok_or_else(|| format!("unknown label {name}"))?;
        let rel = i32::try_from(to as i64 - f.end as i64)
            .map_err(|_| format!("label {name} is too far away"))?;
        e.bytes[f.at..f.at + 4].copy_from_slice(&rel.to_le_bytes());
    }
    e.fixups = data;
    Ok(())
}
