pub mod print;
mod rewrite;
mod wrap;

use crate::ast::program::{Item, Program, RecordEntry};
use crate::ast::sexpr;
use crate::ast::stmt::Stmt;
use crate::ast::{ItemType, Type, type_text};
use crate::diag::Diagnostic;
use crate::source::Source;
use print::{note_lines, stmts};
use wrap::wrap;

/// The canonical source text of a program.
pub fn format_program(program: &Program) -> String {
    let program = &rewrite::rewrite(program);
    let mut lines: Vec<String> = Vec::new();
    let mut attached = false;
    for item in &program.items {
        if !lines.is_empty() && !attached {
            lines.push(String::new());
        }
        attached = matches!(item, Item::Note { attached: true, .. });
        lines.extend(item_lines(item));
    }
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// Hands back `formatted` only if reading it again gives the program that was meant to be
/// written (the original with the spec's rewrites), ignoring ids, lines and blank lines.
pub fn verified(program: &Program, formatted: String) -> Result<String, Diagnostic> {
    let again = crate::parse_source(&Source::new(&formatted));
    match again {
        Ok(read) if shape(&read) == shape(&rewrite::rewrite(program)) => Ok(formatted),
        _ => Err(Diagnostic::program(
            "format would change this program; please report it",
        )),
    }
}

/// The program as text that leaves out ids, lines and blank-line flags.
fn shape(program: &Program) -> String {
    let body = |stmts: &[Stmt]| stmts.iter().map(sexpr::stmt).collect::<Vec<_>>().join(" ");
    let item = |item: &Item| match item {
        Item::Note { note, .. } => format!("{note:?}"),
        Item::Record(r) => {
            let entries = r.entries.iter().map(|e| match e {
                RecordEntry::Field(f) => format!("{} {:?}", f.name, f.ty),
                RecordEntry::Note(note, _) => format!("{note:?}"),
            });
            format!(
                "record {} {}",
                r.name,
                entries.collect::<Vec<_>>().join(" ")
            )
        }
        Item::Table(t) => format!("table {} {} {:?}", t.name, t.record, t.path),
        Item::Feature(f) => {
            let needs = f
                .needs
                .iter()
                .map(|n| format!("{} {}", n.name, type_text(&n.ty)));
            let needs: Vec<String> = needs.collect();
            let before = f.notes_before_needs;
            format!(
                "feature {} {} {before} {}",
                f.name,
                needs.join(" "),
                body(&f.body)
            )
        }
        Item::Test(t) => format!("test {:?} {}", t.title, body(&t.body)),
    };
    program
        .items
        .iter()
        .map(item)
        .collect::<Vec<_>>()
        .join("\n")
}

fn item_lines(item: &Item) -> Vec<String> {
    match item {
        Item::Note { note, .. } => note_lines(note, 0),
        Item::Record(r) => {
            let mut lines = vec![format!("record {}", r.name)];
            for (i, entry) in r.entries.iter().enumerate() {
                let (blank, text) = match entry {
                    RecordEntry::Field(f) => {
                        let ty = type_text(&Type::Item(ItemType::Plain(f.ty)));
                        (f.blank_before, vec![format!("  {} as {ty}", f.name)])
                    }
                    RecordEntry::Note(note, blank) => (*blank, note_lines(note, 1)),
                };
                if blank && i > 0 {
                    lines.push(String::new());
                }
                lines.extend(text);
            }
            lines
        }
        Item::Table(t) => vec![format!(
            "table {} of {} in \"{}\"",
            t.name,
            t.record,
            t.path.replace('"', "\"\"")
        )],
        Item::Feature(f) => {
            let mut lines = vec![format!("feature {}", f.name)];
            let (notes, rest) = f.body.split_at(f.notes_before_needs);
            lines.extend(stmts(notes, 1));
            if !f.needs.is_empty() {
                let inputs: Vec<String> = f
                    .needs
                    .iter()
                    .map(|i| format!("{} as {}", i.name, type_text(&i.ty)))
                    .collect();
                lines.extend(wrap(1, &format!("needs {}", inputs.join(", "))));
                // A blank line before the first statement is kept, as between any two lines.
                if rest.first().is_some_and(|s| s.blank_before) {
                    lines.push(String::new());
                }
            }
            lines.extend(stmts(rest, 1));
            lines
        }
        Item::Test(t) => {
            let mut lines = vec![format!("test \"{}\"", t.title.replace('"', "\"\""))];
            lines.extend(stmts(&t.body, 1));
            lines
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::expenses;

    fn fmt(s: &str) -> String {
        crate::format_source(s).unwrap()
    }

    #[test]
    fn canonical_layout() {
        let messy = "note top\nfeature start\n  set  total  to 1+2\n\n\n\n  show total ,\"x\"\nrecord E\n  a as text\n";
        assert_eq!(
            fmt(messy),
            "note top\nfeature start\n  set total to 1 + 2\n\n  show total, \"x\"\n\nrecord E\n  a as text\n"
        );
    }

    #[test]
    fn a_format_that_would_change_the_program_is_refused() {
        let program = parsed("feature start\n  show 1\n  set t to 1\n  set t to t + 2\n  show t\n");
        let good = crate::format::format_program(&program);
        assert_eq!(super::verified(&program, good.clone()), Ok(good));
        let wrong = "feature start\n  show 2\n  set t to 1\n  add 2 to t\n  show t\n";
        let err = super::verified(&program, wrong.into()).unwrap_err();
        assert_eq!(
            err.problem,
            "format would change this program; please report it"
        );
        assert!(super::verified(&program, "feature start\n  show\n".into()).is_err());
    }

    #[test]
    fn a_blank_line_after_needs_is_kept() {
        let kept = "feature f\n  needs n as number\n\n  show n\n";
        assert_eq!(fmt(kept), kept);
        let noted = "feature f\n  note why\n  needs n as number\n\n  show n\n";
        assert_eq!(fmt(noted), noted);
        let squeezed = "feature f\n  needs n as number\n\n\n\n  show n\n";
        assert_eq!(fmt(squeezed), kept);
        let none = "feature f\n  needs n as number\n  show n\n";
        assert_eq!(fmt(none), none);
    }

    #[test]
    fn idempotent_on_complete_example() {
        let example = expenses();
        assert_eq!(fmt(&example), example);
        assert_eq!(fmt(&fmt(&example)), fmt(&example));
    }

    #[test]
    fn rewrites() {
        assert_eq!(
            fmt("feature start\n  set t to 1\n  set t to t + 2\n  set t to 3 * t\n  show t\n"),
            "feature start\n  set t to 1\n  add 2 to t\n  multiply t by 3\n  show t\n"
        );
        assert_eq!(
            fmt("feature start\n  set d to yes\n  if d is no\n    show 1\n"),
            "feature start\n  set d to yes\n  if not d\n    show 1\n"
        );
    }

    fn parsed(s: &str) -> crate::ast::program::Program {
        crate::parse_source(&crate::source::Source::new(s)).unwrap()
    }

    #[test]
    fn wraps_long_lines_after_commas() {
        let long = format!(
            "feature start\n  show {}\n",
            (1..=30)
                .map(|i| format!("\"item{i:02}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let out = fmt(&long);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.lines().nth(2).unwrap().starts_with("    \"item"));
        parsed(&out);
        assert_eq!(fmt(&out), fmt(&long));
        assert_eq!(fmt(&out), out);
    }

    #[test]
    fn never_breaks_inside_text_or_loops_on_a_long_item() {
        let item = format!("\"{}, b\"", "a".repeat(120));
        let src = format!("feature start\n  show \"a, b\", {item}, \"c, d\"\n");
        let out = fmt(&src);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "  show \"a, b\",");
        assert_eq!(lines[2], format!("    {item},"));
        assert_eq!(lines[3], "    \"c, d\"");
        assert_eq!(fmt(&out), out);
    }

    #[test]
    fn width_counts_characters_not_bytes() {
        let items = (1..=12)
            .map(|i| format!("\"éé{i:02}éééé\""))
            .collect::<Vec<_>>();
        let out = fmt(&format!("feature start\n  show {}\n", items.join(", ")));
        let widths: Vec<usize> = out.lines().skip(1).map(|l| l.chars().count()).collect();
        assert!(widths.iter().all(|w| *w <= 100), "{out}");
        assert!(widths[0] >= 90, "{out}");
        assert!(out.lines().nth(1).unwrap().len() > 100, "bytes exceed 100");
    }

    #[test]
    fn formatting_is_stable_and_keeps_meaning() {
        let long = (1..=40)
            .map(|i| format!("n{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let rewrites =
            "feature f\n  needs a as number\n  set a to a + 1\n  if a is yes\n    show 1\n";
        let wrapped = format!("feature g\n  set l to list of {long}\n  run f with {long}\n");
        let nots = "feature f\n  if not d is no\n    show 1\n  if a and not e is no\n    show 2\n";
        let parens =
            "feature f\n  if not (d is yes)\n    show 1\n  if (d is yes) and e\n    show 2\n";
        for src in [
            expenses(),
            rewrites.into(),
            wrapped,
            nots.into(),
            parens.into(),
        ] {
            let out = fmt(&src);
            assert_eq!(super::format_program(&parsed(&out)), out);
            assert_eq!(fmt(&out), out);
        }
    }

    #[test]
    fn a_rewrite_inside_not_never_prints_not_not() {
        assert_eq!(
            fmt("feature f\n  if not d is no\n    show 1\n"),
            "feature f\n  if d\n    show 1\n"
        );
        assert_eq!(
            fmt("feature f\n  if a and not e is no\n    show 1\n"),
            "feature f\n  if a and e\n    show 1\n"
        );
    }

    #[test]
    fn a_rewrite_that_leaves_a_bare_value_drops_its_parentheses() {
        assert_eq!(
            fmt("feature f\n  if not (d is yes)\n    show 1\n  if (d is yes) and e\n    show 2\n"),
            "feature f\n  if not d\n    show 1\n  if d and e\n    show 2\n"
        );
        let own = "feature f\n  show (d)\n";
        assert_eq!(fmt(own), own);
    }

    #[test]
    fn a_note_before_needs_stays_before_it() {
        let s = "feature f\n  note why\n  needs a as number\n  show a\n";
        assert_eq!(fmt(s), s);
        let two =
            "feature f\n  note one\n  note\n    two\n  needs a as number\n  note after\n  show a\n";
        assert_eq!(fmt(two), two);
    }

    #[test]
    fn blank_lines_between_record_fields_are_kept() {
        let s = "record E\n  a as text\n\n  note why b\n  b as number\n  c as text\n";
        assert_eq!(fmt(s), s);
        let many = "record E\n  a as text\n\n\n\n  b as number\n";
        assert_eq!(fmt(many), "record E\n  a as text\n\n  b as number\n");
    }

    #[test]
    fn note_blocks_round_trip() {
        let s = "note\n  Walks the list.\n\n    Indented more.\nfeature start\n  show 1\n";
        assert_eq!(fmt(s), s);
    }
}
