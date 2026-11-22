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

#[test]
fn a_tab_or_carriage_return_inside_a_test_title_is_rejected() {
    let tab = first_error("feature start\n  show 1\ntest \"a\tb\"\n  show 1\n");
    assert!(tab.starts_with("line 3: "), "{tab}");
    assert!(tab.contains("Tabs are not allowed; use spaces"), "{tab}");
    let cr = first_error("feature start\n  show 1\ntest \"a\rb\"\n  show 1\n");
    assert!(
        cr.contains("A carriage return is not allowed inside text"),
        "{cr}"
    );
}

#[test]
fn places_is_one_digit_from_zero_to_four_written_as_such() {
    let bad = |places: &str| {
        first_error(&format!(
            "feature start\n  show 1 as text with {places} places\n"
        ))
    };
    for places in ["2.0", "02", "5", "10", "2.5"] {
        let out = bad(places);
        assert!(
            out.starts_with(&format!("line 2: show 1 as text with {places} places. The number of places is a digit from 0 to 4")),
            "{out}"
        );
    }
    let ok = "feature start\n  show 1 as text with 4 places\n";
    assert!(format_source(ok).is_ok());
}

#[test]
fn an_empty_test_title_is_refused() {
    let out = first_error("feature start\n  show 1\ntest \"\"\n  show 1\n");
    assert_eq!(
        out,
        "line 3: test \"\". A test title is a sentence in quotes."
    );
}

const LOWEST: &str = "922337203685477.5808";
const RANGE: &str = "This number is outside the range Gloss can hold";

#[test]
fn the_most_negative_number_is_a_literal_with_its_minus() {
    let text = format!("feature start\n  show -{LOWEST}\n");
    assert!(check_source(&text).is_empty());
    assert_eq!(format_source(&text).unwrap(), text);
    let spaced = format!("feature start\n  show - {LOWEST}\n");
    assert_eq!(format_source(&spaced).unwrap(), text);
}

#[test]
fn the_most_negative_magnitude_alone_is_out_of_range() {
    for show in [
        LOWEST.to_string(),
        format!("3 - {LOWEST}"),
        "922337203685477.5809".into(),
    ] {
        let out = first_error(&format!("feature start\n  show {show}\n"));
        assert!(out.contains(RANGE), "{show}: {out}");
    }
    let past = first_error("feature start\n  show -922337203685477.5809\n");
    assert!(past.contains(RANGE), "{past}");
}

#[test]
fn a_list_after_in_needs_parentheses() {
    let out = first_error("feature start\n  for each i in list of 1\n    show i\n");
    assert_eq!(
        out,
        "line 2: for each i in list of 1. Expected a value but found list; list of … goes in parentheses here, as in for each i in (list of 1)."
    );
    let ok = "feature start\n  for each i in (list of 1)\n    show i\n";
    assert_eq!(format_source(ok).unwrap(), ok);
}
