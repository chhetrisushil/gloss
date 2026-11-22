use super::Parser;
use crate::ast::PlainType;
use crate::ast::program::*;
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
        if !self.eat_kw(Kw::Record) {
            let all = "Only record, table, feature, test and note can start a line";
            return Err(self.err(format!("{all} at the top level")));
        }
        Ok(Item::Record(self.record(line)?))
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
            let line = self.line();
            let name = self.parse_name("a field")?;
            self.expect_word("a field", Kw::As)?;
            let ty = self.plain_type()?;
            self.end_line()?;
            entries.push(RecordEntry::Field(FieldDecl { line, name, ty }));
        }
        self.close_block();
        Ok(RecordDecl {
            line,
            name,
            entries,
        })
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
    use crate::source::Source;

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
