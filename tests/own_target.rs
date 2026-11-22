//! A `set` that combines its own target with one value is written with a verb.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

#[test]
fn a_set_on_its_own_target_is_caught_through_parentheses() {
    let rule = |value: &str| {
        render(&format!(
            "feature start\n  set t to 1\n  set t to {value}\n  show t\n"
        ))
    };
    for value in ["t + 1", "(t) + 1", "1 + (t)", "((t)) + 1"] {
        let out = rule(value);
        assert_eq!(out.len(), 1, "{value}: {out:?}");
        assert!(
            out[0].contains("set with + on its own target is written with add"),
            "{out:?}"
        );
    }
    assert!(rule("(t + 1)").is_empty());
    let item = render(
        "feature start\n  set xs to list of 1\n  set item (1) of xs to item 1 of xs + 1\n  show xs\n",
    );
    assert_eq!(item.len(), 1, "{item:?}");
}
