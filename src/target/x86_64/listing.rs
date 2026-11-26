//! The assembly as a target-neutral listing.

use super::inst::{Mem, R, Sym, X, text};
use super::linux::syscall_name;
use super::program::Assembly;
use crate::ir::Module;
use crate::target::listing::{Item, Ref, Routine};

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

pub fn listing(a: &Assembly, m: &Module) -> Result<Vec<Routine>, String> {
    let mut out = Vec::new();
    for r in &a.routines {
        let mut items = Vec::new();
        // Helpers have no IR function, so no slots or inputs.
        let (slots, params) = m
            .functions
            .iter()
            .find(|f| f.name == r.name)
            .map_or((0, 0), |f| (i64::from(f.slots), i64::from(f.params)));
        let mut rax = None;
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
                X::Jcc(c, l) => items.push(jump(&r.name, l, |l| X::Jcc(*c, l))),
                X::Jmp(l) => items.push(jump(&r.name, l, X::Jmp)),
                x => {
                    // The number is known only from a `movabs rax, N` that nothing has
                    // touched since: any other instruction forgets it, `push` aside.
                    match x {
                        X::MovRI(R::Rax, n) => rax = Some(*n),
                        // The `syscall` itself reads the number below.
                        X::Push(_) | X::Syscall => {}
                        _ => rax = None,
                    }
                    let mut refs: Vec<Ref> = mem(x)
                        .into_iter()
                        .filter_map(|m| mem_ref(m, slots, params))
                        .collect();
                    if matches!(x, X::Syscall) {
                        let name = rax
                            .take()
                            .and_then(syscall_name)
                            .ok_or_else(|| format!("syscall in {} has no name", r.name))?;
                        refs.push(Ref::System(name));
                    }
                    items.push(Item::Inst {
                        text: text(x),
                        refs,
                    });
                }
            }
        }
        out.push(Routine {
            name: r.name.clone(),
            items,
        });
    }
    Ok(out)
}

fn jump(name: &str, label: &str, make: impl Fn(String) -> X) -> Item {
    Item::Inst {
        text: text(&make(operand(name, label))),
        refs: local(name, label).map(Ref::Jump).into_iter().collect(),
    }
}

/// The memory operand of an instruction, if it has one.
fn mem(x: &X) -> Option<&Mem> {
    match x {
        X::MovRM(_, m) | X::MovMR(m, _) | X::MovzxRM8(_, m) | X::MovM8R(m, _) | X::Lea(_, m) => {
            Some(m)
        }
        _ => None,
    }
}

/// Slot `k` is `[rbp-8k]`; argument `k` of `p` is `[rbp+16+8(p-k)]`. The saved `b` word
/// below the slots is neither.
fn mem_ref(m: &Mem, slots: i64, params: i64) -> Option<Ref> {
    match m {
        Mem::Rip(Sym::Text(k)) => Some(Ref::Text(*k)),
        Mem::Base(R::Rbp, d) => {
            let d = i64::from(*d);
            if d < 0 && d % 8 == 0 && (1..=slots).contains(&(-d / 8)) {
                Some(Ref::Slot((-d / 8) as u32))
            } else if d >= 16
                && (d - 16) % 8 == 0
                && (1..=params).contains(&(params - (d - 16) / 8))
            {
                Some(Ref::Input((params - (d - 16) / 8) as u32))
            } else {
                None
            }
        }
        _ => None,
    }
}
