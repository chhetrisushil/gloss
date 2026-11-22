//! A line after a give, stop or skip never runs.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

#[test]
fn a_line_after_a_give_is_a_warning_and_the_feature_is_not_missing_a_give() {
    let out = render("feature f\n  give 1\n  show 2\nfeature start\n  show (f)\n");
    assert_eq!(
        out,
        ["line 3: warning: show 2. this line never runs; the give on line 2 ends the feature."]
    );
}

#[test]
fn only_the_first_line_after_the_give_is_reported() {
    let out = render("feature f\n  give 1\n  show 2\n  show 3\nfeature start\n  show (f)\n");
    assert_eq!(out.len(), 1, "{out:?}");
}

#[test]
fn a_line_after_stop_or_skip_in_a_loop_is_a_warning() {
    let body = |word: &str| {
        render(&format!(
            "feature start\n  for each i from 1 to 3\n    {word}\n    show i\n"
        ))
    };
    assert_eq!(
        body("stop"),
        ["line 4: warning: show i. this line never runs; the stop on line 3 ends the loop."]
    );
    let skip = body("skip");
    assert_eq!(skip.len(), 1, "{skip:?}");
    assert!(skip[0].contains("the skip on line 3 moves on to the next pass of the loop"));
}

#[test]
fn a_give_in_the_middle_of_a_block_counts_as_giving() {
    let out = render(
        "feature f\n  needs n as number\n  if n is 1\n    give 1\n    show 2\n  otherwise\n    give 2\n  show 3\nfeature start\n  show (f with 1)\n",
    );
    assert!(out.iter().all(|d| d.contains("warning")), "{out:?}");
    assert!(
        out.iter().all(|d| !d.contains("can reach its end")),
        "{out:?}"
    );
}

#[test]
fn a_line_after_an_if_whose_every_branch_gives_is_a_warning() {
    let out = render(
        "feature f\n  needs n as number\n  if n is 1\n    give 1\n  otherwise\n    give 2\n  show 3\nfeature start\n  show (f with 1)\n",
    );
    assert_eq!(
        out,
        [
            "line 7: warning: show 3. this line never runs; every branch of the if on line 3 ends the feature."
        ]
    );
}

#[test]
fn a_line_after_an_if_whose_every_branch_stops_or_skips_is_a_warning() {
    let out = render(
        "feature start\n  for each i from 1 to 3\n    if i is 1\n      stop\n    otherwise if i is 2\n      skip\n    otherwise\n      stop\n    show i\n",
    );
    assert_eq!(
        out,
        [
            "line 9: warning: show i. this line never runs; every branch of the if on line 3 ends the loop."
        ]
    );
}

#[test]
fn an_if_with_a_branch_that_continues_or_no_otherwise_is_quiet() {
    let open = "feature start\n  for each i from 1 to 3\n    if i is 1\n      stop\n    otherwise\n      show i\n    show i\n";
    assert!(render(open).is_empty(), "{:?}", render(open));
    let bare = "feature start\n  for each i from 1 to 3\n    if i is 1\n      stop\n    show i\n";
    assert!(render(bare).is_empty());
}
