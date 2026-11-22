pub mod print;

use crate::ast::program::{Item, Program, RecordEntry};
use crate::ast::{ItemType, Type, type_text};
use print::{note_lines, stmts};

/// The canonical source text of a program.
pub fn format_program(program: &Program) -> String {
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

fn item_lines(item: &Item) -> Vec<String> {
    match item {
        Item::Note { note, .. } => note_lines(note, 0),
        Item::Record(r) => {
            let mut lines = vec![format!("record {}", r.name)];
            for entry in &r.entries {
                match entry {
                    RecordEntry::Field(f) => {
                        let ty = type_text(&Type::Item(ItemType::Plain(f.ty)));
                        lines.push(format!("  {} as {ty}", f.name));
                    }
                    RecordEntry::Note(note) => lines.extend(note_lines(note, 1)),
                }
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
            if !f.needs.is_empty() {
                let inputs: Vec<String> = f
                    .needs
                    .iter()
                    .map(|i| format!("{} as {}", i.name, type_text(&i.ty)))
                    .collect();
                lines.push(format!("  needs {}", inputs.join(", ")));
            }
            lines.extend(stmts(&f.body, 1));
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
    fn idempotent_on_complete_example() {
        let example = expenses();
        assert_eq!(fmt(&example), example);
        assert_eq!(fmt(&fmt(&example)), fmt(&example));
    }

    #[test]
    fn note_blocks_round_trip() {
        let s = "note\n  Walks the list.\n\n    Indented more.\nfeature start\n  show 1\n";
        assert_eq!(fmt(s), s);
    }
}
