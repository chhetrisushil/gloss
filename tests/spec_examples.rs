//! Guard: every ```gloss block in the spec must compile as its kind demands.

use gloss::parser::parse_stmts_str;
use gloss::source::Source;
use gloss::{check_source, format_source, parse_source};

const SPEC: &str = include_str!("../spec/v0.md");

/// The text of every block whose opening fence is ```gloss, possibly indented
/// inside a list item; the fence's indent is stripped from each block line.
fn gloss_blocks() -> Vec<String> {
    blocks_in(SPEC)
}

fn blocks_in(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<(usize, String)> = None;
    let mut other_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        match &mut current {
            // Inside some other fenced block, a ```gloss line is just text.
            None if other_fence => other_fence = trimmed != "```",
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
            None => other_fence = trimmed.starts_with("```"),
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
    let top_starts = ["record ", "table ", "feature ", "test ", "note "];
    if block.contains("use \"") {
        Class::Future
    } else if block.contains("feature start") {
        Class::Program
    } else if first == "note" || top_starts.iter().any(|s| first.starts_with(s)) {
        Class::Top
    } else {
        Class::Snippet
    }
}

/// First lines of program blocks that show a warning on purpose; none do today.
const INTENTIONAL_WARNINGS: &[&str] = &[];

/// A block of top-level items made self-contained: the record and table of the running
/// example and its `category_total` feature (unless the block is one of them), and a
/// `start` that loads the table.
fn in_context(block: &str) -> String {
    let blocks = gloss_blocks();
    let find = |prefix: &str| {
        let found = blocks.iter().find(|b| b.starts_with(prefix));
        found.unwrap_or_else(|| panic!("no spec block starts with {prefix:?}"))
    };
    let parts = [find("record Expense"), find("feature category_total")];
    let mut text = String::new();
    for part in parts.iter().filter(|p| p.as_str() != block) {
        text.push_str(&format!("{part}\n"));
    }
    text.push_str(&format!(
        "{block}\nfeature start\n  load expenses\n  show 1\n"
    ));
    text
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
            let warnings: Vec<_> = diags.iter().filter(|d| !d.is_error()).collect();
            assert!(
                warnings.is_empty() || INTENTIONAL_WARNINGS.contains(&first),
                "{name}: warnings: {warnings:?}"
            );
            let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();
            assert!(errors.is_empty(), "{name}: check errors: {errors:?}");
            assert_formats_to_itself(&name, block);
        }
        Class::Top => {
            if let Err(d) = parse_source(&Source::new(block)) {
                panic!("{name}: parse failed: {d:?}");
            }
            assert_formats_to_itself(&name, block);
            let errors: Vec<_> = check_source(&in_context(block))
                .into_iter()
                .filter(|d| d.is_error())
                .collect();
            assert!(errors.is_empty(), "{name}: check errors: {errors:?}");
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

#[test]
fn extraction_ignores_gloss_fences_inside_other_blocks() {
    let md = "```\n```gloss\nfeature a\n```\n\n```gloss\nfeature b\n```\n";
    assert_eq!(blocks_in(md), ["feature b\n"]);
    assert_eq!(classify("notebook x\n"), Class::Snippet);
    assert_eq!(classify("note\n  text\n"), Class::Top);
    assert_eq!(classify("note a\n"), Class::Top);
}

/// Every compile message the spec quotes, with a small program that makes the compiler say it.
/// The spec leaves out the echoed source line, so only the message itself is compared.
const QUOTED_MESSAGES: &[(&str, &str)] = &[
    (
        "n stands for an item of numbers and cannot be set. To change the item, loop with for each i from 1 to count of numbers and set item i of numbers. To work on a copy, set m to n first.",
        "feature start\n  set numbers to list of 1\n  for each n in numbers\n    set n to 0\n",
    ),
    (
        "a may not have a value here; it is first set inside the if on line 5. Set it before the if.",
        "feature start\n  set x to 1\n  show x\n  show x\n  if x is 1\n    set a to 1\n  show a\n",
    ),
    (
        "totl has no value. Did you mean total?",
        "feature start\n  set total to 1\n  show total, totl\n",
    ),
    (
        "totl is set but never used",
        "feature start\n  set totl to 1\n",
    ),
    (
        "new name totl; did you mean total?",
        "feature start\n  set total to 1\n  set totl to 2\n  show total, totl\n",
    ),
    (
        "whole of a / b can be read two ways. Write (whole of a) / b or whole of (a / b).",
        "feature start\n  set a to 7\n  set b to 2\n  show whole of a / b\n",
    ),
    (
        "expenses is saved here but nothing in this program loads it. Add load expenses before the first change, or clear expenses if you mean to start the file again.",
        "record E\n  a as number\ntable expenses of E in \"e.csv\"\nfeature start\n  save expenses\n",
    ),
    (
        "category_total can reach its end without giving a value",
        "feature category_total\n  needs n as number\n  if n is 1\n    give 1\nfeature start\n  show (category_total with 1)\n",
    ),
    (
        "cannot work out what countdown gives; add a give that does not call countdown",
        "feature countdown\n  needs n as number\n  give countdown with n\nfeature start\n  show (countdown with 1)\n",
    ),
    (
        "set total as 0. set needs to. Did you mean set total to 0?",
        "feature start\n  set total as 0\n  show total\n",
    ),
    (
        "category_total gives a number; use it, as in set result to category_total with \"food\"",
        "feature category_total\n  needs kind as text\n  give 1\nfeature start\n  run category_total with \"food\"\n",
    ),
];

/// Quoted messages that only a running program says, so there is nothing to compile.
const ILLUSTRATIVE: &[&str] = &[
    "expenses.csv not found; starting with an empty table",
    "expenses.csv line 7: amount \"12,50\" is not a number",
    "line 12: failure 1: dividing by zero (people is 0)",
];

#[test]
fn every_quoted_message_is_what_the_compiler_says() {
    for (quoted, program) in QUOTED_MESSAGES {
        assert!(
            SPEC.contains(quoted),
            "the spec no longer quotes {quoted:?}"
        );
        let said: Vec<String> = check_source(program).iter().map(|d| d.render()).collect();
        let wanted = quoted.trim_end_matches('.');
        assert!(
            said.iter().any(|m| m.contains(wanted)),
            "the compiler does not say {quoted:?}; it says {said:?}"
        );
    }
    for quoted in ILLUSTRATIVE {
        assert!(
            SPEC.contains(quoted),
            "the spec no longer quotes {quoted:?}"
        );
    }
    // Each message the spec quotes with a "line N:" lead is accounted for above: the 10
    // compile messages that have one, and the run-time failure that is illustrative.
    assert_eq!(
        SPEC.matches("\"line ").count(),
        11,
        "a quoted message was added or removed"
    );
}
