//! Ambiguity messages keep the whole expression around the fact that can be read two ways.

use gloss::parser::parse_expr_str;

fn problem(text: &str) -> String {
    parse_expr_str(text).unwrap_err().problem
}

#[test]
fn context_on_both_sides_is_kept() {
    assert_eq!(
        problem("n + 2 * -whole of n / 2 + 5"),
        "whole of n / 2 can be read two ways. Write n + 2 * -(whole of n) / 2 + 5 or n + 2 * -whole of (n / 2) + 5"
    );
}

#[test]
fn text_after_the_ambiguous_part_is_kept() {
    assert_eq!(
        problem("2 * whole of n + 1 + 7"),
        "whole of n + 1 can be read two ways. Write 2 * (whole of n) + 1 + 7 or 2 * whole of (n + 1) + 7"
    );
}

#[test]
fn a_remainder_keeps_the_text_around_it() {
    assert_eq!(
        problem("a + remainder of n by 3 * 2 - c"),
        "remainder of n by 3 * 2 can be read two ways. Write a + (remainder of n by 3) * 2 - c or a + remainder of n by (3 * 2) - c"
    );
}

#[test]
fn the_short_form_stays_short() {
    assert_eq!(
        problem("whole of a / b"),
        "whole of a / b can be read two ways. Write (whole of a) / b or whole of (a / b)"
    );
}

#[test]
fn the_whole_condition_is_kept() {
    assert_eq!(
        problem("x is 1 and n + whole of m / 2 is 3"),
        "whole of m / 2 can be read two ways. Write x is 1 and n + (whole of m) / 2 is 3 or x is 1 and n + whole of (m / 2) is 3"
    );
}
