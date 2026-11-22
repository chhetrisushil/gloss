use super::Parser;
use crate::ast::program::*;
use crate::ast::stmt::{Note, StmtKind};
use crate::ast::{PlainType, Type};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Tok, Token};
use crate::source::Source;

/// Parses a whole program from its tokens.
pub fn parse(toks: &[Token], src: &Source) -> Result<Program, Diagnostic> {
    let mut parser = Parser::new(toks, src);
    if parser.peek() == Some(&Tok::Indent) {
        return Err(parser.err("The first line of a program cannot be indented"));
    }
    let mut items = Vec::new();
    while parser.peek().is_some() {
        items.push(parser.parse_item()?);
    }
    Ok(Program { items })
}

impl Parser<'_> {
    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        let line = self.line();
        if let Some(note) = self.eat_note() {
            let attached = self.toks.get(self.pos).is_some_and(|t| !t.blank_before);
            return Ok(Item::Note {
                line,
                note,
                attached,
            });
        }
        let word = match self.peek() {
            Some(Tok::Kw(k @ (Kw::Record | Kw::Table | Kw::Feature | Kw::Test))) => *k,
            _ => {
                let all = "Only record, table, feature, test and note can start a line";
                return Err(self.err(format!("{all} at the top level")));
            }
        };
        self.pos += 1;
        Ok(match word {
            Kw::Record => Item::Record(self.record(line)?),
            Kw::Table => Item::Table(self.table(line)?),
            Kw::Feature => Item::Feature(self.feature(line)?),
            _ => Item::Test(self.test(line)?),
        })
    }

    /// Consumes a note and the end of its line, if one is next.
    fn eat_note(&mut self) -> Option<Note> {
        let note = match self.peek()? {
            Tok::Note(text) => Note::Line(text.clone()),
            Tok::NoteBlock(lines) => Note::Block(lines.clone()),
            _ => return None,
        };
        self.pos += 1;
        self.end_line().ok()?;
        Some(note)
    }

    fn end_line(&mut self) -> Result<(), Diagnostic> {
        match self.peek() {
            Some(Tok::Newline) => self.pos += 1,
            None => {}
            Some(other) => {
                return Err(self.err(format!("Unexpected {other} at the end of the line")));
            }
        }
        Ok(())
    }

    fn record(&mut self, line: u32) -> Result<RecordDecl, Diagnostic> {
        let name = self.record_name("record")?;
        self.open_block("record")?;
        let mut entries = Vec::new();
        while !matches!(self.peek(), Some(Tok::Dedent) | None) {
            if let Some(note) = self.eat_note() {
                entries.push(RecordEntry::Note(note));
                continue;
            }
            let line = self.line();
            let name = self.parse_name("a field")?;
            self.expect_word("a field", Kw::As)?;
            let ty = self.plain_type()?;
            self.end_line()?;
            entries.push(RecordEntry::Field(FieldDecl { line, name, ty }));
        }
        self.close_block();
        if !entries.iter().any(|e| matches!(e, RecordEntry::Field(_))) {
            let problem = format!("record {name} needs at least one field under it");
            return Err(Diagnostic::error(self.src, line, problem));
        }
        Ok(RecordDecl {
            line,
            name,
            entries,
        })
    }

    fn table(&mut self, line: u32) -> Result<TableDecl, Diagnostic> {
        let name = self.parse_name("table")?;
        self.expect_word("table", Kw::Of)?;
        let record = self.record_name("table")?;
        self.expect_word("table", Kw::In)?;
        let Some(Tok::Text(path)) = self.peek().cloned() else {
            return Err(self.table_path_error(&name));
        };
        self.pos += 1;
        self.end_line()?;
        Ok(TableDecl {
            line,
            name,
            record,
            path,
        })
    }

    /// Quotes the word that stands where the file name belongs, or, when there
    /// is none, a file name made from the table's own name.
    fn table_path_error(&self, name: &str) -> Diagnostic {
        let Some(found) = self.peek().filter(|t| **t != Tok::Newline) else {
            let example = self.example_line(self.pos, &format!("\"{name}.csv\""), false);
            return self.err(format!(
                "table needs a file name in quotes, as in {example}"
            ));
        };
        let example = self.example_line(self.pos, &format!("\"{found}\""), true);
        let problem = "The file name is written in quotes";
        self.err(problem).suggest(example)
    }

    fn plain_type(&mut self) -> Result<PlainType, Diagnostic> {
        let ty = match self.peek() {
            Some(Tok::Kw(Kw::Number)) => PlainType::Number,
            Some(Tok::Kw(Kw::Text)) => PlainType::Text,
            Some(Tok::Kw(Kw::Truth)) => PlainType::Truth,
            _ => return Err(self.err("A record field is a number, text or truth")),
        };
        self.pos += 1;
        Ok(ty)
    }

    fn feature(&mut self, line: u32) -> Result<FeatureDecl, Diagnostic> {
        let name = match self.peek().cloned() {
            Some(Tok::RecordName(wrong)) => {
                let lowered = wrong.to_lowercase().chars().next().into_iter();
                let fixed: String = lowered.chain(wrong.chars().skip(1)).collect();
                let example = self.example_line(self.pos, &fixed, true);
                let problem = "Feature names start with a lowercase letter";
                return Err(self.err(problem).suggest(example));
            }
            _ => self.parse_name("feature")?,
        };
        self.open_block("feature")?;
        let (mut needs, mut body) = (Vec::new(), Vec::new());
        while !matches!(self.peek(), Some(Tok::Dedent) | None) {
            if self.peek() != Some(&Tok::Kw(Kw::Needs)) {
                body.push(self.parse_stmt()?);
                continue;
            }
            let only_notes = body.iter().all(|s| matches!(s.kind, StmtKind::Note(_)));
            if !only_notes || !needs.is_empty() {
                return Err(self.err("needs must be the first line of a feature"));
            }
            self.pos += 1;
            needs = self.inputs()?;
        }
        self.close_block();
        if body.is_empty() {
            let problem = format!("feature {name} needs at least one statement under it");
            return Err(Diagnostic::error(self.src, line, problem));
        }
        Ok(FeatureDecl {
            line,
            name,
            needs,
            body,
        })
    }

    fn inputs(&mut self) -> Result<Vec<InputDecl>, Diagnostic> {
        let mut inputs = Vec::new();
        loop {
            let line = self.line();
            let name = self.parse_name("needs")?;
            self.expect_word("needs", Kw::As)?;
            let ty = if self.eat_kw(Kw::List) {
                self.expect_word("list", Kw::Of)?;
                Type::List(self.parse_item_type()?)
            } else {
                Type::Item(self.parse_item_type()?)
            };
            inputs.push(InputDecl { line, name, ty });
            if !self.eat_sym(crate::lexer::Sym::Comma) {
                break;
            }
        }
        self.end_line()?;
        Ok(inputs)
    }

    fn test(&mut self, line: u32) -> Result<TestDecl, Diagnostic> {
        let Some(Tok::Text(title)) = self.peek().cloned() else {
            let rest = self.toks[self.pos..]
                .iter()
                .take_while(|t| t.tok != Tok::Newline)
                .map(|t| t.tok.to_string())
                .collect::<Vec<_>>();
            let problem = "A test title is a sentence in quotes";
            return Err(self
                .err(problem)
                .suggest(format!("test \"{}\"", rest.join(" "))));
        };
        self.pos += 1;
        let body = self.parse_block("test")?;
        Ok(TestDecl { line, title, body })
    }

    /// A capitalised name; a lowercase one is answered with the capitalised form.
    fn record_name(&mut self, verb: &str) -> Result<String, Diagnostic> {
        match self.peek().cloned() {
            Some(Tok::RecordName(name)) => {
                self.pos += 1;
                Ok(name)
            }
            Some(Tok::Name(wrong)) => {
                let first = wrong.chars().next().map(|c| c.to_ascii_uppercase());
                let fixed: String = first.into_iter().chain(wrong.chars().skip(1)).collect();
                let example = self.example_line(self.pos, &fixed, true);
                let problem = "Record names start with a capital letter";
                Err(self.err(problem).suggest(example))
            }
            _ => Err(self.err(format!("{verb} needs a record name after it"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::program::*;
    use crate::ast::stmt::Note;
    use crate::ast::{ItemType, PlainType, Type};
    use crate::source::Source;

    /// The complete example in section 13 of the language spec.
    fn expenses() -> String {
        let spec = include_str!("../../spec/v0.md");
        let after = spec
            .split("## 13. Complete example")
            .nth(1)
            .expect("spec §13 heading");
        let fenced = after
            .split("```gloss\n")
            .nth(1)
            .expect("spec §13 gloss fence");
        fenced.split("```").next().unwrap().to_string()
    }

    fn parse(t: &str) -> Result<Program, crate::diag::Diagnostic> {
        crate::parse_source(&Source::new(t))
    }

    #[test]
    fn records() {
        let p = parse("record S\n  name as text\n  n as truth\nrecord T\n  a as number\n").unwrap();
        assert_eq!(p.items.len(), 2);
        let r = p.records().next().unwrap();
        assert_eq!((r.line, r.name.as_str()), (1, "S"));
        let fields: Vec<_> = r.fields().map(|f| (f.line, f.name.as_str())).collect();
        assert_eq!(fields, [(2, "name"), (3, "n")]);
    }

    #[test]
    fn notes_at_the_top_and_in_records() {
        let text = "note a\nnote b\nrecord S\n  note the name\n  name as text\n\nnote c\n\nrecord T\n  n as truth\n";
        let p = parse(text).unwrap();
        assert_eq!(p.items.len(), 5);
        let attached = |i: usize| match &p.items[i] {
            Item::Note { attached, .. } => *attached,
            other => panic!("not a note: {other:?}"),
        };
        assert!(attached(0) && attached(1));
        assert!(!attached(3));
        let r = p.records().next().unwrap();
        assert_eq!(r.entries.len(), 2);
        assert_eq!(
            r.entries[0],
            RecordEntry::Note(Note::Line("the name".into()))
        );
        assert!(matches!(p.items[3], Item::Note { line: 7, .. }));
    }

    #[test]
    fn tables() {
        let p = parse("table s of S in \"s.csv\"\n").unwrap();
        let t = p.tables().next().unwrap();
        let table = (t.name.as_str(), t.record.as_str(), t.path.as_str());
        assert_eq!(table, ("s", "S", "s.csv"));
    }

    #[test]
    fn parses_complete_example() {
        let p = parse(&expenses()).unwrap();
        assert_eq!(p.items.len(), 11);
        let r = p.records().next().unwrap();
        let fields: Vec<_> = r.fields().map(|f| f.name.as_str()).collect();
        assert_eq!(fields, ["name", "amount", "category"]);
        let t = p.tables().next().unwrap();
        let table = (t.name.as_str(), t.record.as_str(), t.path.as_str());
        assert_eq!(table, ("expenses", "Expense", "expenses.csv"));
        let ct = p.features().find(|f| f.name == "category_total").unwrap();
        let wanted = InputDecl {
            line: 11,
            name: "wanted".into(),
            ty: Type::Item(ItemType::Plain(PlainType::Text)),
        };
        assert_eq!(ct.needs, vec![wanted]);
        assert_eq!(p.tests().count(), 4);
        let start = p.features().find(|f| f.name == "start").unwrap();
        assert_eq!(start.body.len(), 9);
        assert!(matches!(
            p.items[0],
            Item::Note {
                attached: false,
                ..
            }
        ));
    }

    #[test]
    fn list_input_types_and_record_notes() {
        let text = "record S\n  note the name\n  name as text\nfeature f\n  note why\n  needs xs as list of S, n as number\n  give n\n";
        let p = parse(text).unwrap();
        let f = p.features().next().unwrap();
        assert_eq!(f.needs[0].ty, Type::List(ItemType::Record("S".into())));
        assert_eq!(f.body.len(), 2);
    }

    #[test]
    fn top_level_errors() {
        let e = |t: &str| parse(t).unwrap_err().render();
        assert_eq!(
            e("show 1\n"),
            "line 1: show 1. Only record, table, feature, test and note can start a line at the top level."
        );
        assert_eq!(
            e("record expense\n  a as text\n"),
            "line 1: record expense. Record names start with a capital letter. Did you mean record Expense?"
        );
        assert_eq!(
            e("record E\n  a\n"),
            "line 2: a. a field needs as, as in a as."
        );
        assert_eq!(
            e("table t of E in data\n"),
            "line 1: table t of E in data. The file name is written in quotes. Did you mean table t of E in \"data\"?"
        );
        let missing = "line 1: table t of E in. table needs a file name in quotes, as in table t of E in \"t.csv\".";
        assert_eq!(e("table t of E in\n"), missing);
        assert_eq!(e("table t of E in"), missing);
        assert_eq!(
            e("table expenses of E in\n"),
            "line 1: table expenses of E in. table needs a file name in quotes, as in table expenses of E in \"expenses.csv\"."
        );
        assert_eq!(
            e("feature F\n  stop\n"),
            "line 1: feature F. Feature names start with a lowercase letter. Did you mean feature f?"
        );
        assert_eq!(
            e("feature f\n  add 1 total\n"),
            "line 2: add 1 total. add needs to, as in add 1 to total."
        );
        assert_eq!(
            e("feature f\n  show 1\n  needs a as number\n"),
            "line 3: needs a as number. needs must be the first line of a feature."
        );
        assert_eq!(
            e("feature f\n  needs a as number\n  needs b as text\n  stop\n"),
            "line 3: needs b as text. needs must be the first line of a feature."
        );
        assert_eq!(
            e("feature f\n  needs a as number\n"),
            "line 1: feature f. feature f needs at least one statement under it."
        );
        assert_eq!(
            e("test category total\n  stop\n"),
            "line 1: test category total. A test title is a sentence in quotes. Did you mean test \"category total\"?"
        );
        assert_eq!(
            e("record E\n  note x\n"),
            "line 1: record E. record E needs at least one field under it."
        );
        assert_eq!(
            e("record E\n  a as list of number\n"),
            "line 2: a as list of number. A record field is a number, text or truth."
        );
    }

    #[test]
    fn first_line_cannot_be_indented() {
        let e = parse("  record E\n").unwrap_err().render();
        assert_eq!(
            e,
            "line 1: record E. The first line of a program cannot be indented."
        );
    }
}
