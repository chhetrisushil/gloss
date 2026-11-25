//! Guard: every ```gloss block in the spec must compile as its kind demands.

mod run_support;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use gloss::parser::parse_stmts_str;
use gloss::source::Source;
use gloss::{check_source, format_source, parse_source};
use run_support::{DEADLINE, TempDir, run_limited, run_within, skip};

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

/// What each complete program in the spec prints, by block index: the spec's "prints "..."" sentences
/// (blocks 4 and 6) and the smallest complete program (block 0), whose `show "hello"` is plain.
const PRINTS: &[(usize, &str)] = &[
    (0, "hello\n"),
    (4, "Top student: Meera with 91\nSecond in list: Ravi\n"),
    (6, "Even numbers: 2\n"),
];

/// Builds `source` as `<name>.gloss` in a fresh folder; returns the build result and the folder.
fn build(name: &str, source: &str) -> (std::process::Output, TempDir, PathBuf) {
    let dir = TempDir::new(&format!("spec-{name}"));
    let file = dir.0.join(format!("{name}.gloss"));
    std::fs::write(&file, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .arg("build")
        .arg(&file)
        .output()
        .unwrap();
    let exe = dir.0.join(name);
    (out, dir, exe)
}

fn has_table(block: &str) -> bool {
    block.lines().any(|l| l.starts_with("table "))
}

#[test]
fn every_complete_spec_program_builds_and_runs() {
    let blocks = gloss_blocks();
    let sentences = SPEC.matches("prints \"").count();
    assert!(
        PRINTS.len() >= sentences,
        "the spec has {sentences} `prints \"` sentences but the table holds {}",
        PRINTS.len()
    );
    let mut built = 0;
    for (index, block) in blocks.iter().enumerate() {
        if classify(block) != Class::Program {
            continue;
        }
        let name = format!("block{index}");
        let (out, dir, exe) = build(&name, block);
        if has_table(block) {
            // A table program needs approval (D25). Without it nothing is built; with
            // `--allow` for its file it builds, and only the build is checked here.
            assert_eq!(out.status.code(), Some(4), "{name}: {out:?}");
            let said = String::from_utf8_lossy(&out.stderr);
            assert!(said.starts_with("Not approved: "), "{name}: {said}");
            assert!(
                !exe.exists(),
                "{name}: an unapproved build wrote an executable"
            );
            let file = dir.0.join(format!("{name}.gloss"));
            let allowed = Command::new(env!("CARGO_BIN_EXE_gloss"))
                .args(["build"])
                .arg(&file)
                .args(["--allow", "expenses.csv"])
                .output()
                .unwrap();
            assert_eq!(allowed.status.code(), Some(0), "{name}: {allowed:?}");
            assert!(exe.exists(), "{name}: the approved build wrote nothing");
            continue;
        }
        let first = block.lines().next().unwrap_or("");
        assert_eq!(out.status.code(), Some(0), "{name} ({first:?}): {out:?}");
        built += 1;
        let Some((_, want)) = PRINTS.iter().find(|(i, _)| *i == index) else {
            panic!("{name} ({first:?}) is a complete program with no expected output");
        };
        // No spec program asks a question, so there is no input.
        let Some(ran) = run_limited(&exe, b"", DEADLINE, &[], None) else {
            continue;
        };
        let (status, stdout, stderr) = ran.unwrap();
        assert_eq!(
            status,
            0,
            "{name} ({first:?}): {}",
            String::from_utf8_lossy(&stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&stdout),
            *want,
            "{name} ({first:?})"
        );
        drop(dir);
    }
    assert_eq!(built, PRINTS.len(), "programs built and checked");
}

/// One minimal program per run-time failure: the number, the message after `line N: failure K: `
/// and the program. `at` is the text of the line that fails; the spec's `line 5` becomes its
/// real line. Every program shows "before" first, which must reach standard output.
struct Failure {
    number: u8,
    message: &'static str,
    at: &'static str,
    program: &'static str,
    input: &'static [u8],
    kib: Option<u64>,
}

const FAILURES: &[Failure] = &[
    f(
        1,
        "dividing by zero (people is 0)",
        "divide total",
        "feature start\n  set total to 120\n  set people to 0\n  show \"before\"\n  divide total by people\n",
    ),
    f(
        2,
        "there is no item 7 in prices; it has 3 items",
        "item 7",
        "feature start\n  set prices to list of 1, 2, 3\n  show \"before\"\n  show item 7 of prices\n",
    ),
    f(
        2,
        "there is no first item in prices; it is empty",
        "first of",
        "feature start\n  set prices to empty list of number\n  show \"before\"\n  show first of prices\n",
    ),
    f(
        3,
        "an item position must be whole (n is 2.5)",
        "item n",
        "feature start\n  set xs to list of 1, 2\n  set n to 2.5\n  show \"before\"\n  show item n of xs\n",
    ),
    f(
        3,
        "the end of a count must be whole (top is 2.5)",
        "for each",
        "feature start\n  set top to 2.5\n  show \"before\"\n  for each i from 1 to top\n    show i\n",
    ),
    f(
        3,
        "the start of a count must be whole (low is 0.5)",
        "for each",
        "feature start\n  set low to 0.5\n  show \"before\"\n  for each i from low to 3\n    show i\n",
    ),
    f(
        4,
        "a number went out of range (price * 1000000)",
        "price *",
        "feature start\n  set price to 922337203685477\n  show \"before\"\n  show price * 1000000\n",
    ),
    f(
        5,
        "\"12a\" is not a number (answer as number)",
        "answer as",
        "feature start\n  set answer to \"12a\"\n  show \"before\"\n  show answer as number\n",
    ),
    Failure {
        input: b"",
        ..f(
            9,
            "input ended while ask was waiting",
            "ask",
            "feature start\n  show \"before\"\n  ask \"Q\" into s\n  show s\n",
        )
    },
    // Out of memory needs a 1 GiB virtual-memory limit under Rosetta (ruling M2-R35).
    Failure {
        kib: Some(1_048_576),
        ..f(
            10,
            "out of memory",
            "append",
            "feature start\n  set xs to empty list of number\n  show \"before\"\n  repeat while yes\n    append 1 to xs\n",
        )
    },
    f(
        10,
        "calls nested too deep (countdown)",
        "give (countdown",
        "feature countdown\n  needs n as number\n  if n is -1\n    give 0\n  give (countdown with n + 1) + 1\n\nfeature start\n  show \"before\"\n  show (countdown with 1)\n",
    ),
];

const fn f(number: u8, message: &'static str, at: &'static str, program: &'static str) -> Failure {
    Failure {
        number,
        message,
        at,
        program,
        input: b"",
        kib: None,
    }
}

#[test]
fn every_run_time_failure_says_what_the_spec_says() {
    let numbers: Vec<u8> = FAILURES.iter().map(|f| f.number).collect();
    for n in [1, 2, 3, 4, 5, 9, 10] {
        assert!(numbers.contains(&n), "no program for failure {n}");
    }
    for (i, fail) in FAILURES.iter().enumerate() {
        let tail = format!("failure {}: {}", fail.number, fail.message);
        // The table writes the first form in full and later forms as `…: <message>`.
        assert!(
            SPEC.contains(&format!("`line 5: {tail}`"))
                || SPEC.contains(&format!("`…: {}`", fail.message))
                || SPEC.contains(&format!("or `…: {}`", fail.message)),
            "the spec no longer words failure {} as {:?}",
            fail.number,
            fail.message
        );
        let line = fail
            .program
            .lines()
            .position(|l| l.contains(fail.at))
            .unwrap()
            + 1;
        let want = format!("line {line}: {tail}\n");
        let name = format!("failure{i}");
        let (out, _dir, exe) = build(&name, fail.program);
        assert_eq!(out.status.code(), Some(0), "{name}: {out:?}");
        let Some(ran) = run_limited(&exe, fail.input, DEADLINE, &[], fail.kib) else {
            continue;
        };
        let (status, stdout, stderr) = ran.unwrap();
        assert_eq!(String::from_utf8_lossy(&stderr), want, "{name}");
        assert_eq!(status, 2, "{name}");
        let shown = String::from_utf8_lossy(&stdout);
        assert!(
            shown.starts_with("before\n"),
            "{name}: standard output {shown:?}"
        );
    }
    // Failures 6 to 8 are about table files: their messages are compared in
    // `table_failure_messages_match_the_design`.
}

const DESIGN: &str = include_str!("../docs/specs/2026-11-22-compiler-design.md");

/// The backtick-quoted spans of one table row.
fn spans(row: &str) -> Vec<&str> {
    row.split('`').skip(1).step_by(2).collect()
}

/// `(number, message)` for every failure 6 to 8 message in the rows of `text` that start with
/// `| N |`. The design writes every
/// message in full as `line N: failure K: ...`; the spec writes the first of a row that way
/// and the rest as `...: <message>`.
fn table_messages(text: &str, is_design: bool) -> Vec<(u8, String)> {
    let mut found = Vec::new();
    for n in 6..=8u8 {
        for row in text.lines().filter(|l| l.starts_with(&format!("| {n} |"))) {
            let lead = if is_design { "line N" } else { "line 5" };
            let full = format!("{lead}: failure {n}: ");
            // Other tables (the task list) have rows `| 6 |` with no failure message.
            let all = spans(row);
            if !all.iter().any(|s| s.starts_with(&full)) {
                continue;
            }
            // The design's cause column may hold backticks; its message is the last span.
            let quoted = if is_design {
                vec![all[all.len() - 1]]
            } else {
                all
            };
            for span in quoted {
                let message = span
                    .strip_prefix(&full)
                    .or_else(|| span.strip_prefix("…: "))
                    .unwrap_or_else(|| panic!("failure {n}: unexpected span {span:?} in {row}"));
                found.push((n, message.to_string()));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn table_failure_messages_match_the_design() {
    let design = table_messages(DESIGN, true);
    // Guard against a vacuous pass: 12 for failure 6, 3 for failure 7, 1 for failure 8.
    assert_eq!(
        design.len(),
        16,
        "the design's failure 6-8 table: {design:?}"
    );
    let spec = table_messages(SPEC, false);
    assert_eq!(
        spec, design,
        "spec section 10 rows 6-8 and the design's table must hold the same messages"
    );
}

/// Whether a block has tests, and the number of them.
fn test_count(block: &str) -> usize {
    block.lines().filter(|l| l.starts_with("test \"")).count()
}

/// Builds `source` as a test executable and runs it with no input; `None` without a runner.
fn run_tests(name: &str, source: &str) -> Option<(i32, String)> {
    let bytes = gloss::build_test_executable(source)
        .unwrap_or_else(|d| panic!("{name}: does not build: {d:?}"));
    let dir = TempDir::new(&format!("spec-tests-{name}"));
    let exe = dir.0.join("tests");
    std::fs::write(&exe, bytes).unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (status, out, _) = run_within(&exe, b"", DEADLINE)?.unwrap();
    Some((status, String::from_utf8(out).unwrap()))
}

#[test]
fn the_spec_13_program_is_the_expense_tracker_case() {
    let block = gloss_blocks()
        .into_iter()
        .find(|b| b.starts_with("note Expense tracker"))
        .expect("the section 13 program");
    let case = include_str!("test_cases/expense_tracker.gloss");
    assert_eq!(block, case, "spec section 13 and the test case differ");
}

#[test]
fn every_complete_spec_program_with_tests_passes() {
    let mut run_programs = 0;
    for (index, block) in gloss_blocks().iter().enumerate() {
        let class = classify(block);
        let tests = test_count(block);
        if tests == 0 || !matches!(class, Class::Program | Class::Top) {
            continue;
        }
        let name = format!("block{index}");
        let source = if class == Class::Program {
            block.clone()
        } else {
            in_context(block)
        };
        let needs_start = block.contains("run start");
        if class == Class::Top && needs_start {
            // These tests run `start`, which the stand-in context does not have: build only.
            gloss::build_test_executable(&source)
                .unwrap_or_else(|d| panic!("{name}: does not build: {d:?}"));
            continue;
        }
        let Some((status, out)) = run_tests(&name, &source) else {
            skip();
            return;
        };
        let want = format!("\n{tests} passed, 0 failed\n");
        assert!(out.ends_with(&want), "{name}: {out}");
        assert_eq!(status, 0, "{name}: {out}");
        run_programs += 1;
    }
    // The section 13 program and the section 9 `category_total` test.
    assert_eq!(run_programs, 2, "spec programs with tests that were run");
}

#[test]
fn the_section_9_test_snippets_parse_check_and_format() {
    let blocks = gloss_blocks();
    let block = blocks
        .iter()
        .find(|b| b.contains("input \"Dinner\", \"300\", \"food\""))
        .expect("the section 9 whole-program test");
    assert!(block.contains("expect first of shown is \"Total for food: 420\""));
    assert_eq!(classify(block), Class::Top);
    check_block(0, block, Class::Top);
}

/// The section 9 report sample is what the real report looks like: a program with the
/// sample's titles, its failed `expect` on line 23 and its one printed line gives it exactly.
#[test]
fn the_section_9_report_sample_is_the_real_report() {
    let sample = SPEC
        .split("```text\n")
        .map(|part| part.split("```").next().unwrap())
        .find(|part| part.starts_with("pass  category total"))
        .expect("the section 9 report sample");
    let head = "record Expense\n  name as text\n  amount as number\n  category as text\n\n\
                feature largest_expense\n  give Expense with name \"Bus\", amount 40, category \"travel\"\n\n\
                feature start\n  show 1\n\n\
                test \"category total counts only the matching category\"\n  expect 1 is 1\n\n\
                test \"largest expense is found\"\n";
    let mut source = head.to_string();
    while source.lines().count() < 21 {
        source.push_str("  note padding so the failing expect is on line 23\n");
    }
    source.push_str("  show \"Largest so far: Bus at 40\"\n");
    source.push_str("  expect name of largest_expense is \"Dinner\"\n\n");
    source.push_str("test \"a cost that is not a number is asked for again\"\n  expect 1 is 1\n");
    assert_eq!(
        source
            .lines()
            .position(|l| l.contains("expect name of largest"))
            .unwrap()
            + 1,
        23
    );
    let Some((status, out)) = run_tests("report-sample", &source) else {
        skip();
        return;
    };
    assert_eq!(out, sample, "the spec's report sample");
    assert_eq!(status, 1);
}
