//! Compares the encoder's output with an objdump listing of the same bytes.

use gloss::target::x86_64::encode::{Encoded, encode};
use gloss::target::x86_64::inst::{X, text};

/// One disassembled instruction.
#[derive(Debug, PartialEq)]
pub struct Line {
    pub addr: usize,
    pub bytes: Vec<u8>,
    pub text: String,
}

/// Reads `objdump -D` output: `   0:\t48 89 c0   \tmov    rax,rax`, with the bytes of a
/// long instruction continued on lines of their own.
pub fn parse(dis: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for ln in dis.lines() {
        let parts: Vec<&str> = ln.split('\t').collect();
        let Some(head) = parts[0].trim().strip_suffix(':') else {
            continue;
        };
        let Ok(addr) = usize::from_str_radix(head, 16) else {
            continue;
        };
        let Some(hex) = parts.get(1) else { continue };
        let bytes: Vec<u8> = hex
            .split_whitespace()
            .filter_map(|b| u8::from_str_radix(b, 16).ok())
            .collect();
        match (parts.get(2), out.last_mut()) {
            (Some(t), _) => out.push(Line {
                addr,
                bytes,
                text: t.to_string(),
            }),
            (None, Some(last)) => last.bytes.extend(bytes),
            (None, None) => {}
        }
    }
    out
}

/// Hex numbers as decimal: 64-bit two's complement, except after a `-`, which is a sign.
fn decimal(t: &str) -> String {
    let mut out = String::new();
    let mut rest = t;
    while let Some(i) = rest.find("0x") {
        out.push_str(&rest[..i]);
        let digits: String = rest[i + 2..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        let n = u64::from_str_radix(&digits, 16).unwrap_or(0);
        if out.ends_with('-') {
            out.push_str(&n.to_string());
        } else {
            out.push_str(&(n as i64).to_string());
        }
        rest = &rest[i + 2 + digits.len()..];
    }
    out + rest
}

/// Drops what is only format: size words, annotations, spaces after the mnemonic.
pub fn norm_objdump(t: &str) -> String {
    let t = t.split('#').next().unwrap_or("");
    let t = t.split('<').next().unwrap_or("");
    let t = t.replace("QWORD PTR ", "").replace("BYTE PTR ", "byte ");
    squash(&decimal(&t))
}

fn squash(t: &str) -> String {
    match t.trim().split_once(char::is_whitespace) {
        Some((m, rest)) => format!("{m} {}", rest.replace(' ', "")),
        None => t.trim().to_string(),
    }
}

/// Our text in the same form. A branch shows the offset of its label, and a `rip`
/// operand a displacement of 0 (the linker patches it later).
fn norm_ours(x: &X, enc: &Encoded) -> String {
    let t = text(x);
    let target = match x {
        X::Jcc(_, l) | X::Jmp(l) | X::Call(l) => enc.labels.get(l),
        _ => None,
    };
    if let Some(at) = target {
        let (m, _) = t.split_once(' ').unwrap_or((&t, ""));
        return format!("{m} {at}");
    }
    let t = match t.split_once("rip+") {
        Some((a, b)) => {
            let end = b.find(']').unwrap_or(b.len());
            format!("{a}rip+0{}", &b[end..])
        }
        None => t,
    };
    squash(&t)
}

/// Checks each instruction of `xs` (labels and comments make no bytes) against `dis`.
/// Returns how many there were and what differed.
pub fn compare(xs: &[X], dis: &str) -> (usize, Vec<String>) {
    let enc = encode(xs).expect("the sweep encodes");
    let lines = parse(dis);
    let real: Vec<&X> = xs
        .iter()
        .filter(|x| !matches!(x, X::Label(_) | X::Comment(_)))
        .collect();
    let mut bad = Vec::new();
    if lines.len() != real.len() {
        bad.push(format!(
            "{} instructions, objdump has {}",
            real.len(),
            lines.len()
        ));
    }
    let mut at = 0;
    for (x, l) in real.iter().zip(&lines) {
        let own = piece(x);
        let want = norm_ours(x, &enc);
        let got = norm_objdump(&l.text);
        if l.addr != at || l.bytes.len() != own.bytes.len() {
            bad.push(format!(
                "{} at {at}: objdump has {} bytes at {}",
                text(x),
                l.bytes.len(),
                l.addr
            ));
        } else if l.bytes != enc.bytes[at..at + own.bytes.len()] {
            bad.push(format!("{} at {at}: bytes {:02x?}", text(x), l.bytes));
        }
        if want != got {
            bad.push(format!(
                "{} at {at}: objdump says {got:?}, expected {want:?}",
                text(x)
            ));
        }
        for f in &own.fixups {
            if f.at + 4 != f.end || f.end != own.bytes.len() {
                bad.push(format!(
                    "{} at {at}: fixup {f:?} is not the last 4 bytes",
                    text(x)
                ));
            }
        }
        at += own.bytes.len();
    }
    if at != enc.bytes.len() {
        bad.push(format!(
            "instructions end at {at}, code at {}",
            enc.bytes.len()
        ));
    }
    (real.len(), bad)
}

/// `x` alone; a branch has its label defined just before so that it resolves.
fn piece(x: &X) -> Encoded {
    let mut v = Vec::new();
    if let X::Jcc(_, l) | X::Jmp(l) | X::Call(l) = x {
        v.push(X::Label(l.clone()));
    }
    v.push(x.clone());
    encode(&v).expect("one instruction encodes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gloss::target::x86_64::inst::R;

    const MOV: &str = "   0:\t48 89 c8             \tmov    rax,rcx\n";

    #[test]
    fn a_listing_that_agrees_has_no_mismatches() {
        assert_eq!(compare(&[X::MovRR(R::Rax, R::Rcx)], MOV), (1, vec![]));
    }

    #[test]
    fn a_deliberately_wrong_listing_is_reported() {
        let wrong = MOV.replace("rax,rcx", "rcx,rax");
        let (n, bad) = compare(&[X::MovRR(R::Rax, R::Rcx)], &wrong);
        assert_eq!((n, bad.len()), (1, 1), "{bad:?}");
        let wrong = MOV.replace("48 89 c8", "48 89 c1");
        assert_eq!(compare(&[X::MovRR(R::Rax, R::Rcx)], &wrong).1.len(), 1);
        assert!(!compare(&[X::Ret, X::Ret], "   0:\tc3\tret\n").1.is_empty());
    }
}
