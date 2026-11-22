//! A diagnostic about a missing value is reported on the line that is missing it.

use gloss::{check_source, format_source};

fn first_error(text: &str) -> String {
    match format_source(text) {
        Err(d) => d.render(),
        Ok(_) => panic!("expected a parse error for {text:?}"),
    }
    .replace('\n', " | ")
}

#[test]
fn a_missing_value_is_reported_on_its_own_line() {
    let out = first_error("feature start\n  show\n  show 2\n");
    assert!(
        out.starts_with("line 2: show. Expected a value but found the end of the line"),
        "{out}"
    );
}

#[test]
fn a_set_with_nothing_after_to_keeps_its_line_before_blank_lines() {
    let out = first_error("feature start\n  set x to\n\n\n  show 2\n");
    assert!(
        out.starts_with("line 2: set x to. Expected a value but found the end of the line"),
        "{out}"
    );
}

#[test]
fn a_missing_value_at_the_end_of_the_file_names_the_last_line() {
    let out = first_error("feature start\n  show 1\n  set x to");
    assert!(
        out.starts_with("line 3: set x to. Expected a value"),
        "{out}"
    );
}

#[test]
fn with_at_the_end_of_a_line_is_reported_on_that_line() {
    let out = first_error("feature start\n  show 1 as text with\n  show 2\n");
    assert!(
        out.starts_with("line 2: show 1 as text with. The number of places"),
        "{out}"
    );
}

#[test]
fn empty_list_of_at_the_end_of_a_line_is_reported_on_that_line() {
    let out = first_error("feature start\n  set x to empty list of\n  show 2\n");
    assert!(
        out.starts_with("line 2: set x to empty list of. Expected"),
        "{out}"
    );
    assert!(check_source("feature start\n  show 1\n").is_empty());
}

#[test]
fn give_inside_a_test_is_an_error() {
    let text = "feature start\n  show 1\ntest \"t\"\n  give 1\n";
    let out: Vec<String> = check_source(text).iter().map(|d| d.render()).collect();
    assert_eq!(
        out,
        ["line 4: give 1. give can only be used inside a feature."]
    );
}
