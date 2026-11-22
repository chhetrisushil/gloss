//! One cause gets one diagnostic.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

const NEEDS_N: &str = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n";

#[test]
fn changing_an_input_to_another_type_gives_one_diagnostic() {
    let out = render(&format!("{NEEDS_N}  set n to \"a\"\n  give n\n"));
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(
        out[0].contains("n is an input, so it cannot be changed"),
        "{out:?}"
    );
}

#[test]
fn changing_a_counting_loop_variable_gives_one_diagnostic() {
    let out = render("feature start\n  for each i from 1 to 3\n    set i to \"a\"\n");
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(
        out[0].contains("i counts the loop and cannot be changed"),
        "{out:?}"
    );
    assert!(!out[0].contains("it is an input"), "{out:?}");
}

#[test]
fn asking_into_an_input_or_loop_variable_gives_one_diagnostic() {
    let input = render(&format!("{NEEDS_N}  ask \"q\" into n\n  give n\n"));
    assert_eq!(input.len(), 1, "{input:?}");
    let looped = render("feature start\n  for each i from 1 to 3\n    ask \"q\" into i\n");
    assert_eq!(looped.len(), 1, "{looped:?}");
}
