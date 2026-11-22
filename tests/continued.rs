//! A statement continued after a comma is echoed whole in its diagnostics.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

#[test]
fn an_error_on_a_continued_statement_echoes_all_of_it() {
    let out = render("feature start\n  show 1,\n    2,\n    x\n");
    assert_eq!(out, ["line 4: show 1, 2, x. x has no value."]);
    let first = render("feature start\n  show y,\n    2\n");
    assert_eq!(first, ["line 2: show y, 2. y has no value."]);
}

#[test]
fn joined_lines_are_single_spaced_and_other_lines_untouched() {
    let out = render("feature start\n  show 1,     \n    2\n  show z\n");
    assert_eq!(out, ["line 4: show z. z has no value."]);
    let both = render("feature start\n  show a,   \n    b\n");
    assert_eq!(both.len(), 2, "{both:?}");
    assert!(both.iter().all(|d| d.contains("show a, b.")), "{both:?}");
}
