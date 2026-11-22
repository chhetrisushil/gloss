//! A table taken as a value is a list; only its own name is a table.

use gloss::check_source;

const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

fn start(lines: &str) -> Vec<String> {
    let text = format!("{HEAD}feature start\n{lines}");
    check_source(&text).iter().map(|d| d.render()).collect()
}

#[test]
fn load_needs_the_name_of_a_table() {
    let out = start("  set mine to es\n  load mine\n  show mine\n");
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(
        out[0].starts_with("line 6: load mine. load works on tables; mine is a list of E"),
        "{out:?}"
    );
}

#[test]
fn a_copied_table_can_be_replaced_by_a_list() {
    let out = start("  set mine to es\n  set mine to empty list of E\n  show mine\n");
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn tables_and_lists_compare_for_equality() {
    let lines =
        "  set l to empty list of E\n  show (es is l)\n  show (es is not l)\n  show (es is es)\n";
    assert!(start(lines).is_empty());
}

#[test]
fn a_feature_can_give_a_table_and_a_list() {
    let text = format!(
        "{HEAD}feature pick\n  needs n as number\n  if n is 1\n    give es\n  give empty list of E\n\
         feature start\n  set got to pick with 1\n  show got\n"
    );
    let out: Vec<String> = check_source(&text).iter().map(|d| d.render()).collect();
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn a_table_and_its_copy_are_accepted_where_a_list_is_needed() {
    let text = format!(
        "{HEAD}feature total\n  needs xs as list of E\n  give count of xs\nfeature start\n  set mine to es\n  show (total with es), (total with mine)\n"
    );
    let out: Vec<String> = check_source(&text).iter().map(|d| d.render()).collect();
    assert!(out.is_empty(), "{out:?}");
}
