//! Opt-in (`GLOSS_OBJDUMP=1`): every instruction the encoder can make is disassembled by
//! GNU objdump and compared with our own text, byte for byte and operand for operand.

#[path = "objdump/cmp.rs"]
mod cmp;
mod run_support;
#[path = "objdump/sweep.rs"]
mod sweep;

use run_support::{TempDir, runner};
use std::io::Write;
use std::process::Command;

#[test]
fn objdump_agrees_with_the_encoder_on_every_instruction() {
    if std::env::var_os("GLOSS_OBJDUMP").is_none_or(|v| v != "1") {
        let _ = std::io::stderr().write_all(b"objdump check skipped: set GLOSS_OBJDUMP=1\n");
        return;
    }
    let Some(runner) = runner() else {
        run_support::skip();
        return;
    };
    let xs = sweep::sweep();
    let enc = gloss::target::x86_64::encode::encode(&xs).expect("the sweep encodes");
    let dir = TempDir::new("objdump");
    let file = dir.0.join("sweep.bin");
    std::fs::write(&file, &enc.bytes).unwrap();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/objdump-check.sh");
    let out = Command::new("sh")
        .arg(script)
        .arg(&file)
        .env("GLOSS_RUNNER", runner.join(" "))
        .output()
        .expect("the script starts");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (n, bad) = cmp::compare(&xs, &String::from_utf8_lossy(&out.stdout));
    let _ = writeln!(
        std::io::stderr(),
        "objdump: {n} instructions, {} mismatches",
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches, first: {:#?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
}
