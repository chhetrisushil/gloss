//! Multiplication, division and remainder run in the machine give exactly the `Number`
//! reference: a status byte (0 ok, 1 out of range, 2 divide by zero), then the 8 result bytes.

mod run_support;

use gloss::ir::Reg::{A, B};
use gloss::ir::{Data, Function, Global, Inst, Label, Module, Prim, Reg};
use gloss::number::{NumFail, Number};
use gloss::target::x86_64_linux;
use run_support::{TempDir, run};
use std::os::unix::fs::PermissionsExt;

/// Zero, the scale and its halves, both signs, and the ends of the range.
fn specials() -> Vec<i64> {
    let small = [1, 5000, 9999, 10000, i64::MAX / 10000, i64::MAX / 10000 + 1];
    let signed = small.iter().flat_map(|&n| [n, -n]);
    signed.chain([0, i64::MIN, i64::MAX]).collect()
}

/// Every special pair, then pseudo-random ones of mixed widths, 300 in all.
fn pairs() -> Vec<(i64, i64)> {
    let all = specials();
    let mut pairs: Vec<(i64, i64)> = all
        .iter()
        .flat_map(|&x| all.iter().map(move |&y| (x, y)))
        .collect();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    while pairs.len() < 300 {
        let mut operand = || (next() as i64) >> (next() % 64);
        pairs.push((operand(), operand()));
    }
    pairs
}

/// What one operation writes: a status byte (0 ok, 1 out of range, 2 divide by zero), the
/// 8 result bytes (0 on failure), then the 8 bytes of `b`, which must be unchanged.
fn expected(result: Result<Number, NumFail>, b: i64) -> [u8; 17] {
    let mut bytes = [0; 17];
    match result {
        Ok(Number(n)) => bytes[1..9].copy_from_slice(&n.to_le_bytes()),
        Err(NumFail::OutOfRange) => bytes[0] = 1,
        Err(NumFail::DivideByZero) => bytes[0] = 2,
    }
    bytes[9..].copy_from_slice(&b.to_le_bytes());
    bytes
}

fn store(offset: i32, from: Reg) -> Inst {
    Inst::Store {
        base: A,
        offset,
        from,
    }
}

/// Computes `op(x, y)`, stores the 17 bytes `expected` describes in `out_buf` and writes
/// them to standard output. `first` is the first free label number.
fn block(x: i64, y: i64, first: u32, op: impl Fn(Label, Label) -> Inst) -> Vec<Inst> {
    let (range, zero, done) = (Label(first), Label(first + 1), Label(first + 2));
    let out = Inst::AddressOf(A, Data::Global(Global::OutBuf));
    let status = |s: i64| {
        let byte = Inst::StoreByte {
            base: A,
            offset: 0,
            from: B,
        };
        let (set, clear) = (Inst::Const(B, s), Inst::Const(B, 0));
        [out.clone(), store(9, B), set, byte, clear, store(1, B)]
    };
    let mut body = vec![Inst::Const(A, x), Inst::Const(B, y), op(range, zero)];
    // On success `a` is the result; `b` is saved while it holds the address' partner.
    body.extend([
        Inst::Push(B),
        Inst::Move { to: B, from: A },
        out.clone(),
        store(1, B),
        Inst::Pop(B),
        store(9, B),
        Inst::Const(B, 0),
        Inst::StoreByte {
            base: A,
            offset: 0,
            from: B,
        },
        Inst::Jump(done),
        Inst::Mark(range),
    ]);
    body.extend(status(1));
    body.extend([Inst::Jump(done), Inst::Mark(zero)]);
    body.extend(status(2));
    body.extend([
        Inst::Mark(done),
        Inst::Const(A, 1),
        Inst::Push(A),
        out.clone(),
        Inst::Push(A),
        Inst::Const(A, 17),
        Inst::Push(A),
        Inst::System(Prim::Write),
    ]);
    body
}

/// Runs `mul`, `div` and `rem` on every pair in the machine and compares with `Number`.
fn check(name: &str, pairs: &[(i64, i64)]) {
    let mut body = vec![Inst::Enter(0)];
    let mut want = Vec::new();
    let mut label = 0;
    for &(x, y) in pairs {
        let (nx, ny) = (Number(x), Number(y));
        body.extend(block(x, y, label, |range, _| Inst::NumMul { range }));
        want.extend(expected(nx.mul_round(ny), y));
        body.extend(block(x, y, label + 3, |range, zero| Inst::NumDiv {
            zero,
            range,
        }));
        want.extend(expected(nx.div_round(ny), y));
        body.extend(block(x, y, label + 6, |_, zero| Inst::NumRem { zero }));
        want.extend(expected(nx.rem(ny), y));
        label += 9;
    }
    body.extend([Inst::Const(A, 0), Inst::Push(A), Inst::System(Prim::Exit)]);
    let mut m = Module::default();
    m.functions.push(Function {
        name: "_start".to_string(),
        params: 0,
        slots: 0,
        body,
    });
    let bytes = x86_64_linux().executable(&m).expect("the module builds");
    let dir = TempDir::new(name);
    let path = dir.0.join("prog");
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    if let Some(ran) = run(&path, b"") {
        let (status, out, err) = ran.unwrap();
        assert_eq!((status, err), (0, Vec::new()));
        assert_eq!(out.len(), want.len());
        for (i, (got, want)) in out.chunks(17).zip(want.chunks(17)).enumerate() {
            let (pair, op) = (pairs[i / 3], ["mul", "div", "rem"][i % 3]);
            assert_eq!(got, want, "{op} of {pair:?}");
        }
    }
}

#[test]
fn multiplication_division_and_remainder_equal_the_reference() {
    check("number-machine", &pairs());
}

/// The reviewer's 61 edge values: the ends of the range, the scale and its halves, and
/// the products and quotients that land on the ends after rounding.
fn edge_values() -> Vec<i64> {
    let (min, max) = (i64::MIN, i64::MAX);
    let mut edge = vec![min, min + 1, min + 2, max, max - 1, max - 2, 0];
    let ns = [
        1,
        2,
        3,
        4999,
        5000,
        5001,
        9999,
        10000,
        10001,
        19999,
        20000,
        15000,
        25000,
        max / 10000,
        max / 10000 + 1,
        max / 10000 - 1,
        1 << 31,
        1 << 32,
        1 << 62,
        (1 << 62) + 1,
        3_037_000_499,
        3_037_000_500,
        303_700_049_997,
        303_700_049_998,
        303_700_050_000,
        min / -10000,
        max / 10000 * 10000,
    ];
    edge.extend(ns.iter().flat_map(|&n: &i64| [n, -n]));
    edge
}

#[test]
fn the_edge_cross_product_equals_the_reference() {
    let edge = edge_values();
    assert_eq!(edge.len(), 61);
    let pairs: Vec<(i64, i64)> = edge
        .iter()
        .flat_map(|&x| edge.iter().map(move |&y| (x, y)))
        .collect();
    assert_eq!(pairs.len(), 3721);
    check("number-edges", &pairs);
}
