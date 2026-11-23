//! 2000 number results computed by one real executable, checked against `Number`.

mod run_support;

use std::process::Command;

use gloss::number::Number;
use run_support::{TempDir, run};

const RESULTS: usize = 2000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
    }

    /// Ten-thousandths, biased towards the values that break arithmetic.
    fn operand(&mut self) -> Number {
        let small = self.below(60_000) - 30_000;
        Number(match self.below(10) {
            0 => 0,
            1 => [10_000, -10_000, 5_000, -5_000, 1, -1][self.below(6) as usize],
            2 => small,
            3 => small * 10_000 / 7,
            4 => i64::MAX - self.below(50_000),
            5 => i64::MIN + self.below(50_000),
            6 => (self.below(2_000_000) - 1_000_000) * 10_000 + self.below(10_000),
            7 => self.next() as i64 / 1_000_000_000,
            _ => self.next() as i64,
        })
    }
}

/// A literal; a negative one is parenthesised so it is a valid operand anywhere.
fn literal(n: Number) -> String {
    match n.show().strip_prefix('-') {
        Some(magnitude) => format!("(-{magnitude})"),
        None => n.show(),
    }
}

/// The source text and expected output line of one operation, if the reference succeeds.
fn operation(rng: &mut Rng) -> Option<(String, String)> {
    let (a, b) = (rng.operand(), rng.operand());
    let (la, lb) = (literal(a), literal(b));
    let (text, want) = match rng.below(6) {
        0 => (format!("{la} + {lb}"), a.checked_add(b)),
        1 => (format!("{la} - {lb}"), a.checked_sub(b)),
        2 => (format!("{la} * {lb}"), a.mul_round(b)),
        3 => (format!("{la} / {lb}"), a.div_round(b)),
        4 => (format!("remainder of {la} by {lb}"), a.rem(b)),
        _ => (format!("whole of {la}"), Ok(a.whole())),
    };
    Some((text, want.ok()?.show()))
}

#[test]
fn a_real_executable_computes_numbers_like_the_reference() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut cases = Vec::new();
    while cases.len() < RESULTS {
        cases.extend(operation(&mut rng));
    }
    let mut source = String::from("feature start\n");
    for (text, _) in &cases {
        source.push_str(&format!("  show {text}\n"));
    }
    let dir = TempDir::new("number-reference");
    let path = dir.0.join("numbers.gloss");
    std::fs::write(&path, source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .arg("build")
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    let Some(ran) = run(&dir.0.join("numbers"), b"") else {
        return;
    };
    let (status, out, err) = ran.unwrap();
    assert_eq!((status, err), (0, Vec::new()));
    let out = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), cases.len(), "line count");
    for (i, ((text, want), got)) in cases.iter().zip(&lines).enumerate() {
        assert_eq!(got, want, "line {}: show {text}", i + 1);
    }
}
