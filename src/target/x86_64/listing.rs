//! The assembly as a target-neutral listing.

use super::inst::{X, text};
use super::program::Assembly;
use crate::ir::Module;
use crate::target::listing::{Item, Routine};

/// The routine's own label `L<name>_<n>` as its number `n`.
pub(super) fn local(name: &str, label: &str) -> Option<u32> {
    let digits = label
        .strip_prefix('L')?
        .strip_prefix(name)?
        .strip_prefix('_')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// A jump target, printed `.L<n>` when it is the routine's own label. Calls are left alone:
/// they name routines, never a routine's own labels.
fn operand(name: &str, label: &str) -> String {
    match local(name, label) {
        Some(n) => format!(".L{n}"),
        None => label.to_string(),
    }
}

pub fn listing(a: &Assembly, _m: &Module) -> Result<Vec<Routine>, String> {
    let mut out = Vec::new();
    for r in &a.routines {
        let mut items = Vec::new();
        for (i, x) in r.code.iter().enumerate() {
            match x {
                X::Comment(c) => {
                    let n = c.strip_prefix("line ").and_then(|n| n.parse().ok());
                    items.push(Item::Line(
                        n.ok_or_else(|| format!("unexpected comment {c:?}"))?,
                    ));
                }
                // The routine's own label: the routine's name already says it.
                X::Label(_) if i == 0 => {}
                X::Label(l) => items.push(match local(&r.name, l) {
                    Some(n) => Item::Label {
                        text: format!(".L{n}"),
                        local: Some(n),
                    },
                    None => Item::Label {
                        text: l.clone(),
                        local: None,
                    },
                }),
                X::Jcc(c, l) => items.push(inst(&X::Jcc(*c, operand(&r.name, l)))),
                X::Jmp(l) => items.push(inst(&X::Jmp(operand(&r.name, l)))),
                x => items.push(inst(x)),
            }
        }
        out.push(Routine {
            name: r.name.clone(),
            items,
        });
    }
    Ok(out)
}

fn inst(x: &X) -> Item {
    Item::Inst {
        text: text(x),
        refs: vec![],
    }
}
