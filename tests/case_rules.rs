//! Names that differ only in case are refused, except a record and a name that differ only
//! in the first letter.

use gloss::check_source;

fn render(text: &str) -> Vec<String> {
    check_source(text).iter().map(|d| d.render()).collect()
}

#[test]
fn two_fields_differing_only_in_case_are_refused() {
    let out =
        render("record E\n  myAmount as number\n  myamount as number\nfeature start\n  show 1\n");
    assert_eq!(
        out,
        ["line 3: myamount as number. myamount and myAmount differ only in case; rename one."]
    );
}

#[test]
fn a_variable_may_differ_from_a_record_only_in_the_first_letter() {
    let record = |name: &str| format!("record {name}\n  a as number\n");
    let body = "feature start\n  set expense to 1\n  show expense\n";
    assert!(render(&format!("{}{body}", record("Expense"))).is_empty());
    let out = render(&format!("{}{body}", record("EXPENSE")));
    assert_eq!(
        out,
        ["line 4: set expense to 1. expense and EXPENSE differ only in case; rename one."]
    );
    let mixed = render(&format!("{}{body}", record("ExPense")));
    assert_eq!(mixed.len(), 1, "{mixed:?}");
}
