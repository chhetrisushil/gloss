//! Checking time stays close to linear. The bound is generous for a debug build.

use gloss::check_source;
use std::time::{Duration, Instant};

fn timed(what: &str, text: &str) -> Vec<String> {
    let started = Instant::now();
    let out: Vec<String> = check_source(text).iter().map(|d| d.render()).collect();
    let took = started.elapsed();
    println!("{what}: {took:?}");
    assert!(took < Duration::from_secs(2), "{what} took {took:?}");
    out
}

#[test]
fn a_long_chain_of_features_declared_caller_first_checks_quickly() {
    let n = 2000;
    let mut text = String::from("feature start\n  show f0\n");
    for i in 0..n {
        let call = if i + 1 < n {
            format!("f{}", i + 1)
        } else {
            "1".into()
        };
        text.push_str(&format!("feature f{i}\n  give {call}\n"));
    }
    let out = timed("2000 features", &text);
    assert!(out.is_empty(), "{:?}", &out[..out.len().min(3)]);
}
