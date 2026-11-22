//! A variable, loop variable or input cannot reuse the name of a feature, table or record.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

#[test]
fn a_variable_cannot_be_named_like_a_feature() {
    let out = render("feature helper\n  give 1\nfeature start\n  set helper to 2\n  show helper\n");
    assert_eq!(
        out,
        ["line 4: set helper to 2. helper is a feature; a variable cannot reuse its name."]
    );
}

#[test]
fn a_loop_variable_cannot_be_named_like_a_table() {
    let out = render(&format!(
        "{HEAD}feature start\n  for each es from 1 to 3\n    show es\n"
    ));
    assert_eq!(
        out,
        ["line 5: for each es from 1 to 3. es is a table; a variable cannot reuse its name."]
    );
}

#[test]
fn an_input_cannot_reuse_a_table_or_feature_name() {
    let table = render(&format!(
        "{HEAD}feature f\n  needs es as number\n  give es\nfeature start\n  show (f with 1)\n"
    ));
    assert_eq!(
        table,
        ["line 5: needs es as number. es is a table; an input cannot reuse its name."]
    );
    let feature = render(
        "feature f\n  needs start as number\n  give start\nfeature start\n  show (f with 1)\n",
    );
    assert_eq!(
        feature,
        ["line 2: needs start as number. start is a feature; an input cannot reuse its name."]
    );
}

#[test]
fn an_ask_cannot_reuse_a_feature_name() {
    let out =
        render("feature helper\n  give 1\nfeature start\n  ask \"q\" into helper\n  show helper\n");
    assert_eq!(
        out,
        ["line 4: ask \"q\" into helper. helper is a feature; a variable cannot reuse its name."]
    );
}
