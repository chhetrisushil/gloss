//! The permission list a person approves at build time: which table files a program can
//! read, or read and replace.

use crate::ast::program::Program;
use crate::ast::stmt::StmtKind;
use crate::ast::walk::for_each_stmt;
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadReplace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub file: String,
    pub access: Access,
}

/// Every declared table's file, in declaration order. ReadReplace when a `save` of
/// that table appears in some feature body (test blocks do not count).
pub fn list(program: &Program) -> Vec<Entry> {
    let mut saved = BTreeSet::new();
    for feature in program.features() {
        for_each_stmt(&feature.body, &mut |s| {
            if let StmtKind::Save(table) = &s.kind {
                saved.insert(table.as_str());
            }
        });
    }
    program
        .tables()
        .map(|t| Entry {
            file: t.path.clone(),
            access: if saved.contains(t.name.as_str()) {
                Access::ReadReplace
            } else {
                Access::ReadOnly
            },
        })
        .collect()
}

/// D24: printable ASCII as is; otherwise `"<name>" (<escaped>)`.
pub fn display_name(file: &str) -> String {
    if file.chars().all(|c| (' '..='~').contains(&c)) {
        return file.to_string();
    }
    let mut escaped = String::new();
    for c in file.chars() {
        if (' '..='~').contains(&c) {
            escaped.push(c);
        } else {
            let _ = write!(escaped, "\\u{{{:X}}}", c as u32);
        }
    }
    format!("\"{file}\" ({escaped})")
}

/// The lines a person sees: "This program can:\n", one line per entry, "  nothing else\n".
pub fn render(entries: &[Entry]) -> String {
    let mut out = String::from("This program can:\n");
    for e in entries {
        let words = match e.access {
            Access::ReadOnly => "read only",
            Access::ReadReplace => "read and replace",
        };
        let _ = writeln!(out, "  {words:<19}{}", display_name(&e.file));
    }
    out.push_str("  nothing else\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    const HEAD: &str =
        "record E\n  amount as number\ntable es of E in \"e.csv\"\ntable rs of E in \"r.csv\"\n";

    fn entries(text: &str) -> Vec<Entry> {
        list(&crate::parse_source(&Source::new(text)).unwrap())
    }

    #[test]
    fn a_saved_table_is_read_and_replace() {
        let text = format!("{HEAD}feature start\n  load es\n  load rs\n  save es\n");
        assert_eq!(
            render(&entries(&text)),
            "This program can:\n  read and replace   e.csv\n  read only          r.csv\n  nothing else\n"
        );
    }

    #[test]
    fn a_save_inside_a_test_does_not_count() {
        let text = format!(
            "{HEAD}table fs of E in \"f.csv\"\nfeature start\n  load es\n  load fs\n  save fs\ntest \"t\"\n  load es\n  save es\n  expect 1 is 1\n"
        );
        let got = entries(&text);
        assert_eq!(got[0].access, Access::ReadOnly);
        assert_eq!(got[1].access, Access::ReadOnly);
        assert_eq!(got[2].access, Access::ReadReplace);
    }

    #[test]
    fn a_save_in_a_loop_and_if_is_found() {
        let text = format!(
            "{HEAD}feature start\n  load es\n  for each i from 1 to 2\n    if i is 1\n      save es\n"
        );
        assert_eq!(entries(&text)[0].access, Access::ReadReplace);
    }

    #[test]
    fn a_save_in_another_feature_counts() {
        let text =
            format!("{HEAD}feature start\n  load es\n  run other\nfeature other\n  save es\n");
        assert_eq!(entries(&text)[0].access, Access::ReadReplace);
    }

    #[test]
    fn a_nested_save_in_a_feature_counts() {
        let text = format!("{HEAD}feature start\n  load es\n  if 1 is 1\n    save es\n");
        assert_eq!(entries(&text)[0].access, Access::ReadReplace);
    }

    #[test]
    fn no_tables_lists_nothing_else() {
        assert_eq!(render(&[]), "This program can:\n  nothing else\n");
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("expenses.csv"), "expenses.csv");
        assert_eq!(display_name("data/a b.csv"), "data/a b.csv");
        assert_eq!(
            display_name("résumé.csv"),
            "\"résumé.csv\" (r\\u{E9}sum\\u{E9}.csv)"
        );
        assert_eq!(
            display_name("d\u{430}ta.csv"),
            "\"d\u{430}ta.csv\" (d\\u{430}ta.csv)"
        );
        assert_eq!(
            display_name("a\u{1F600}.csv"),
            "\"a\u{1F600}.csv\" (a\\u{1F600}.csv)"
        );
    }

    #[test]
    fn checked_permissions_match_list() {
        let text = format!("{HEAD}feature start\n  load es\n  save es\n");
        let (diags, checked) = crate::check_program(&text);
        assert!(diags.iter().all(|d| !d.is_error()), "{diags:?}");
        let program = crate::parse_source(&Source::new(&text)).unwrap();
        assert_eq!(checked.unwrap().permissions(), list(&program));
    }
}
