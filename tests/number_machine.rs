//! Multiplication, division and remainder run in the machine give exactly the `Number`
//! reference: a status byte (0 ok, 1 out of range, 2 divide by zero), then the 8 result bytes.

mod run_support;

use gloss::ir::Reg::{A, B};
use gloss::ir::{Data, Function, Global, Inst, Label, Module, Prim};
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

fn expected(result: Result<Number, NumFail>) -> [u8; 9] {
    let mut bytes = [0; 9];
    match result {
        Ok(Number(n)) => bytes[1..].copy_from_slice(&n.to_le_bytes()),
        Err(NumFail::OutOfRange) => bytes[0] = 1,
        Err(NumFail::DivideByZero) => bytes[0] = 2,
    }
    bytes
}

/// Computes `op(x, y)`, stores the status byte and the result in `out_buf` (0 on failure)
/// and writes those 9 bytes to standard output. `first` is the first free label number.
fn block(x: i64, y: i64, first: u32, op: impl Fn(Label, Label) -> Inst) -> Vec<Inst> {
    let (range, zero, done) = (Label(first), Label(first + 1), Label(first + 2));
    let store_status = |status: i64| {
        vec![
            Inst::Const(B, status),
            Inst::AddressOf(A, Data::Global(Global::OutBuf)),
            Inst::StoreByte {
                base: A,
                offset: 0,
                from: B,
            },
            Inst::Const(B, 0),
            Inst::Store {
                base: A,
                offset: 1,
                from: B,
            },
            Inst::Jump(done),
        ]
    };
    let mut body = vec![Inst::Const(A, x), Inst::Const(B, y), op(range, zero)];
    body.extend([
        Inst::Move { to: B, from: A },
        Inst::AddressOf(A, Data::Global(Global::OutBuf)),
        Inst::Store {
            base: A,
            offset: 1,
            from: B,
        },
        Inst::Const(B, 0),
        Inst::StoreByte {
            base: A,
            offset: 0,
            from: B,
        },
        Inst::Jump(done),
        Inst::Mark(range),
    ]);
    body.extend(store_status(1));
    body.push(Inst::Mark(zero));
    body.extend(store_status(2));
    body.extend([
        Inst::Mark(done),
        Inst::Const(A, 1),
        Inst::Push(A),
        Inst::AddressOf(A, Data::Global(Global::OutBuf)),
        Inst::Push(A),
        Inst::Const(A, 9),
        Inst::Push(A),
        Inst::System(Prim::Write),
    ]);
    body
}

#[test]
fn multiplication_division_and_remainder_equal_the_reference() {
    let mut body = vec![Inst::Enter(0)];
    let mut want = Vec::new();
    for (x, y) in pairs() {
        let (nx, ny) = (Number(x), Number(y));
        let n = body.len() as u32 * 3;
        body.extend(block(x, y, n, |range, _| Inst::NumMul { range }));
        want.extend(expected(nx.mul_round(ny)));
        body.extend(block(x, y, n + 1_000_000, |range, zero| Inst::NumDiv {
            zero,
            range,
        }));
        want.extend(expected(nx.div_round(ny)));
        body.extend(block(x, y, n + 2_000_000, |_, zero| Inst::NumRem { zero }));
        want.extend(expected(nx.rem(ny)));
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
    let dir = TempDir::new("number-machine");
    let path = dir.0.join("prog");
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    if let Some(ran) = run(&path, b"") {
        let (status, out, err) = ran.unwrap();
        assert_eq!((status, err), (0, Vec::new()));
        assert_eq!(out.len(), want.len());
        for (i, (got, want)) in out.chunks(9).zip(want.chunks(9)).enumerate() {
            let (pair, op) = (pairs()[i / 3], ["mul", "div", "rem"][i % 3]);
            assert_eq!(got, want, "{op} of {pair:?}");
        }
    }
}
