//! Guard: every ```gloss block in the spec must compile as its kind demands.

use gloss::parser::parse_stmts_str;
use gloss::source::Source;
use gloss::{check_source, format_source, parse_source};

const SPEC: &str = include_str!("../spec/v0.md");

/// The text of every block whose opening fence is ```gloss, possibly indented
/// inside a list item; the fence's indent is stripped from each block line.
fn gloss_blocks() -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for line in SPEC.lines() {
        let trimmed = line.trim_start();
        match &mut current {
            None if trimmed == "```gloss" => {
                current = Some((line.len() - trimmed.len(), String::new()));
            }
            Some((_, text)) if trimmed == "```" => {
                blocks.push(std::mem::take(text));
                current = None;
            }
            Some((indent, text)) => {
                let pad = " ".repeat(*indent);
                text.push_str(line.strip_prefix(pad.as_str()).unwrap_or(line.trim_start()));
                text.push('\n');
            }
            None => {}
        }
    }
    assert!(
        current.is_none(),
        "a gloss block in the spec is never closed"
    );
    blocks
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    Future,
    Program,
    Top,
    Snippet,
}

fn classify(block: &str) -> Class {
    let first = block.lines().next().unwrap_or("");
    let top_starts = ["record ", "table ", "feature ", "test ", "note"];
    if block.contains("use \"") {
        Class::Future
    } else if block.contains("feature start") {
        Class::Program
    } else if top_starts.iter().any(|s| first.starts_with(s)) {
        Class::Top
    } else {
        Class::Snippet
    }
}

fn assert_formats_to_itself(name: &str, block: &str) {
    match format_source(block) {
        Ok(out) => assert_eq!(out, block, "{name}: format changed the block"),
        Err(d) => panic!("{name}: format failed: {d:?}"),
    }
}

fn check_block(index: usize, block: &str, class: Class) {
    let first = block.lines().next().unwrap_or("");
    let name = format!("spec block {index} ({class:?}, first line {first:?})");
    match class {
        Class::Future => {}
        Class::Program => {
            let diags = check_source(block);
            for d in diags.iter().filter(|d| !d.is_error()) {
                eprintln!("{name}: warning: {d:?}");
            }
            let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();
            assert!(errors.is_empty(), "{name}: check errors: {errors:?}");
            assert_formats_to_itself(&name, block);
        }
        Class::Top => {
            if let Err(d) = parse_source(&Source::new(block)) {
                panic!("{name}: parse failed: {d:?}");
            }
            assert_formats_to_itself(&name, block);
        }
        Class::Snippet => {
            if let Err(d) = parse_stmts_str(block) {
                panic!("{name}: statements failed to parse: {d:?}");
            }
        }
    }
}

#[test]
fn every_spec_example_compiles() {
    let blocks = gloss_blocks();
    let mut counts = [0usize; 4];
    for (index, block) in blocks.iter().enumerate() {
        let class = classify(block);
        counts[class as usize] += 1;
        check_block(index, block, class);
    }
    assert!(
        blocks.len() >= 10,
        "only {} gloss blocks found",
        blocks.len()
    );
    // [future, program, top, snippet]: adding or removing an example must
    // update these numbers deliberately.
    assert_eq!(counts, [1, 4, 4, 1], "blocks per class");
}
