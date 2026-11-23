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

/// `name` with every character outside printable ASCII written as `\u{HEX}`.
fn escape(name: &str) -> String {
    let mut escaped = String::new();
    for c in name.chars() {
        if (' '..='~').contains(&c) {
            escaped.push(c);
        } else {
            let _ = write!(escaped, "\\u{{{:X}}}", c as u32);
        }
    }
    escaped
}

/// D24: printable ASCII as is; otherwise `"<name>" (<escaped>)`.
pub fn display_name(file: &str) -> String {
    if file.chars().all(|c| (' '..='~').contains(&c)) {
        return file.to_string();
    }
    format!("\"{file}\" ({})", escape(file))
}

fn entry_line(e: &Entry) -> String {
    let words = match e.access {
        Access::ReadOnly => "read only",
        Access::ReadReplace => "read and replace",
    };
    format!("  {words:<19}{}\n", display_name(&e.file))
}

/// The lines a person sees: "This program can:\n", one line per entry, "  nothing else\n".
pub fn render(entries: &[Entry]) -> String {
    let mut out = String::from("This program can:\n");
    for e in entries {
        out.push_str(&entry_line(e));
    }
    out.push_str("  nothing else\n");
    out
}

/// The file body for `<source>.allow` (D28).
pub fn allow_file(source_name: &str, entries: &[Entry]) -> String {
    let mut out = format!(
        "note Files this program may touch; approved for {}.\n",
        escape(source_name)
    );
    for e in entries {
        out.push_str(&entry_line(e));
    }
    out
}

/// The approvals a `.allow` text records, as (display-name, access) pairs.
/// Err((line, problem)) for a line that is not the note, blank, or an entry line.
pub fn parse_allow(text: &str) -> Result<Vec<(String, Access)>, (usize, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        if raw.starts_with("note ") {
            continue;
        }
        let line = raw.trim_end_matches([' ', '\r']).trim_start_matches(' ');
        if line.is_empty() {
            continue;
        }
        let entry = [
            ("read and replace", Access::ReadReplace),
            ("read only", Access::ReadOnly),
        ]
        .iter()
        .find_map(|(words, access)| {
            let rest = line.strip_prefix(words)?;
            let name = rest.strip_prefix(' ')?.trim_start_matches(' ');
            (!name.is_empty()).then(|| (name.to_string(), *access))
        });
        match entry {
            Some(pair) => out.push(pair),
            None => {
                return Err((
                    i + 1,
                    "this line is not an approval; expected read only or read and replace and a file name"
                        .to_string(),
                ))
            }
        }
    }
    Ok(out)
}

/// The entries that need a fresh approval, given the recorded approvals and the
/// `--allow` files (raw names as written in table lines). In list order.
pub fn needing_approval<'a>(
    entries: &'a [Entry],
    recorded: &[(String, Access)],
    flags: &[String],
) -> Vec<&'a Entry> {
    entries
        .iter()
        .filter(|e| {
            let name = display_name(&e.file);
            !flags.contains(&e.file)
                && !recorded.iter().any(|(n, a)| {
                    *n == name && (*a == Access::ReadReplace || e.access == Access::ReadOnly)
                })
        })
        .collect()
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

    fn e(file: &str, access: Access) -> Entry {
        Entry {
            file: file.to_string(),
            access,
        }
    }

    #[test]
    fn allow_file_round_trips() {
        let es = [
            e("expenses.csv", Access::ReadOnly),
            e("résumé.csv", Access::ReadReplace),
        ];
        let text = allow_file("expenses.gloss", &es);
        assert_eq!(
            text,
            "note Files this program may touch; approved for expenses.gloss.\n  read only          expenses.csv\n  read and replace   \"résumé.csv\" (r\\u{E9}sum\\u{E9}.csv)\n"
        );
        assert_eq!(
            parse_allow(&text).unwrap(),
            vec![
                ("expenses.csv".to_string(), Access::ReadOnly),
                (display_name("résumé.csv"), Access::ReadReplace),
            ]
        );
    }

    #[test]
    fn a_read_only_approval_does_not_cover_replace() {
        let es = [e("a.csv", Access::ReadReplace)];
        let rec = [("a.csv".to_string(), Access::ReadOnly)];
        assert_eq!(needing_approval(&es, &rec, &[]), vec![&es[0]]);
    }

    #[test]
    fn a_flag_covers_any_access() {
        let es = [
            e("a.csv", Access::ReadReplace),
            e("b.csv", Access::ReadOnly),
        ];
        let flags = ["a.csv".to_string()];
        assert_eq!(needing_approval(&es, &[], &flags), vec![&es[1]]);
    }

    #[test]
    fn a_recorded_replace_covers_read_only() {
        let es = [e("résumé.csv", Access::ReadOnly)];
        let rec = [(display_name("résumé.csv"), Access::ReadReplace)];
        assert!(needing_approval(&es, &rec, &[]).is_empty());
    }

    #[test]
    fn an_unknown_recorded_file_is_ignored() {
        let es = [e("a.csv", Access::ReadOnly)];
        let rec = parse_allow("note x\n  read only          gone.csv\n").unwrap();
        assert_eq!(needing_approval(&es, &rec, &[]), vec![&es[0]]);
    }

    #[test]
    fn a_bad_line_names_its_number() {
        assert_eq!(
            parse_allow("note x\nread sometimes  e.csv\n"),
            Err((
                2,
                "this line is not an approval; expected read only or read and replace and a file name"
                    .to_string()
            ))
        );
        assert!(parse_allow("read only\n").is_err());
        assert!(parse_allow("read onlyx.csv\n").is_err());
    }

    #[test]
    fn blank_lines_and_the_note_are_skipped() {
        let got = parse_allow("note anything\n\nread and replace a b.csv \r\n\n").unwrap();
        assert_eq!(got, vec![("a b.csv".to_string(), Access::ReadReplace)]);
    }

    #[test]
    fn a_hostile_source_name_cannot_start_a_line() {
        let es = [e("a.csv", Access::ReadOnly)];
        let text = allow_file("x\nread and replace   data.csv", &es);
        assert_eq!(text.lines().filter(|l| l.starts_with("note ")).count(), 1);
        assert_eq!(text.lines().count(), 2);
        assert_eq!(
            parse_allow(&text).unwrap(),
            vec![("a.csv".to_string(), Access::ReadOnly)]
        );
    }
}
