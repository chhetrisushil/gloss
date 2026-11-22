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
