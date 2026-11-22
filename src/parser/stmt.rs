use super::Parser;
use crate::ast::stmt::{Note, Stmt, StmtKind as S, Target};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Tok, Token, lex};
use crate::source::Source;

/// Parses statements until the end of the text, at level 0.
pub fn parse_stmts_str(text: &str) -> Result<Vec<Stmt>, Diagnostic> {
    let src = Source::new(text);
    let toks = lex(&src)?;
    let mut parser = Parser::new(&toks, &src);
    let mut stmts = Vec::new();
    while parser.peek().is_some() {
        stmts.push(parser.parse_stmt()?);
    }
    Ok(stmts)
}

impl Parser<'_> {
    pub fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let line = self.line();
        let blank_before = self.toks.get(self.pos).is_some_and(|t| t.blank_before);
        let (kind, is_block) = self.stmt_kind()?;
        match self.peek() {
            _ if is_block => {}
            Some(Tok::Newline) => self.pos += 1,
            None => {}
            Some(other) => {
                return Err(self.err(format!("Unexpected {other} after the statement")));
            }
        }
        Ok(Stmt {
            line,
            blank_before,
            kind,
        })
    }

    /// Parses one statement's content; the flag is true for a block statement, which
    /// ends with its block rather than with a newline.
    fn stmt_kind(&mut self) -> Result<(S, bool), Diagnostic> {
        match self.peek().cloned() {
            Some(Tok::Note(text)) => {
                self.pos += 1;
                Ok((S::Note(Note::Line(text)), false))
            }
            Some(Tok::NoteBlock(lines)) => {
                self.pos += 1;
                Ok((S::Note(Note::Block(lines)), false))
            }
            Some(Tok::Kw(Kw::If | Kw::Otherwise | Kw::For | Kw::Repeat)) => {
                Ok((self.block_stmt()?, true))
            }
            Some(Tok::Kw(word)) => {
                self.pos += 1;
                Ok((self.keyword_stmt(word)?, false))
            }
            Some(other) => Err(self.not_a_statement(other)),
            None => Err(self.err("Expected a statement")),
        }
    }

    fn keyword_stmt(&mut self, word: Kw) -> Result<S, Diagnostic> {
        let verb = word.as_str();
        match word {
            Kw::Set => {
                let target = self.parse_target(verb)?;
                if self.peek() == Some(&Tok::Kw(Kw::As)) {
                    return Err(self.err("set needs to").suggest(self.fixed_line("to")));
                }
                self.expect_word(verb, Kw::To)?;
                let value = self.parse_expr()?;
                Ok(S::Set { target, value })
            }
            Kw::Add => {
                let value = self.parse_arith()?;
                self.expect_word(verb, Kw::To)?;
                let target = self.parse_target(verb)?;
                Ok(S::Add { value, target })
            }
            Kw::Subtract => {
                let value = self.parse_arith()?;
                self.expect_word(verb, Kw::From)?;
                let target = self.parse_target(verb)?;
                Ok(S::Subtract { value, target })
            }
            Kw::Multiply | Kw::Divide => {
                let target = self.parse_target(verb)?;
                self.expect_word(verb, Kw::By)?;
                let by = self.parse_arith()?;
                Ok(if word == Kw::Multiply {
                    S::Multiply { target, by }
                } else {
                    S::Divide { target, by }
                })
            }
            Kw::Append => {
                let value = self.parse_value()?;
                self.expect_word(verb, Kw::To)?;
                let list = self.parse_name(verb)?;
                Ok(S::Append { value, list })
            }
            Kw::Show => Ok(S::Show(self.comma_list()?)),
            Kw::Input => Ok(S::Input(self.comma_list()?)),
            Kw::Ask => {
                let question = self.parse_arith()?;
                self.expect_word(verb, Kw::Into)?;
                let into = self.parse_name(verb)?;
                Ok(S::Ask { question, into })
            }
            Kw::Skip => Ok(S::Skip),
            Kw::Stop => Ok(S::Stop),
            Kw::Give => Ok(S::Give(self.parse_expr()?)),
            Kw::Expect => Ok(S::Expect(self.parse_expr()?)),
            Kw::Run => self.run_stmt(),
            Kw::Load => Ok(S::Load(self.parse_name(verb)?)),
            Kw::Save => Ok(S::Save(self.parse_name(verb)?)),
            Kw::Clear => Ok(S::Clear(self.parse_name(verb)?)),
            other => Err(self.not_a_statement(other.as_str())),
        }
    }

    fn not_a_statement(&self, found: impl std::fmt::Display) -> Diagnostic {
        self.err(format!(
            "Every statement starts with a keyword, such as set or show; {found} is not one"
        ))
    }

    fn run_stmt(&mut self) -> Result<S, Diagnostic> {
        let feature = self.parse_name("run")?;
        let mut inputs = Vec::new();
        if self.eat_kw(Kw::With) {
            if matches!(self.peek(), Some(Tok::Newline) | None) {
                return Err(self.err("with needs at least one input after it"));
            }
            inputs = self.comma_list()?;
        }
        Ok(S::Run { feature, inputs })
    }

    fn parse_target(&mut self, verb: &str) -> Result<Target, Diagnostic> {
        if self.eat_kw(Kw::Item) {
            let position = self.position()?;
            if !self.eat_kw(Kw::Of) {
                return Err(self.err("item needs of after its position"));
            }
            let of = Box::new(self.nested(|p| p.parse_target(verb))?);
            return Ok(Target::Item {
                position: Box::new(position),
                of,
            });
        }
        let name = self.parse_name(verb)?;
        if !self.eat_kw(Kw::Of) {
            return Ok(Target::Var(name));
        }
        let of = Box::new(self.nested(|p| p.parse_target(verb))?);
        Ok(Target::Field { field: name, of })
    }

    /// A lowercase name; the verb is only used in the message.
    pub(super) fn parse_name(&mut self, verb: &str) -> Result<String, Diagnostic> {
        match self.peek().cloned() {
            Some(Tok::Name(name)) => {
                self.pos += 1;
                Ok(name)
            }
            Some(Tok::RecordName(name)) => Err(self.capital_name_error(verb, &name)),
            _ => Err(self.err(format!("{verb} needs a name after it"))),
        }
    }

    /// Lowercases the offending token, wherever it is in the statement.
    fn capital_name_error(&self, verb: &str, name: &str) -> Diagnostic {
        let mut chars = name.chars();
        let first = chars.next().map(|c| c.to_ascii_lowercase());
        let lowered: String = first.into_iter().chain(chars).collect();
        let role = match verb {
            "run" => "Feature",
            "load" | "save" | "clear" | "append" => "Table and list",
            _ => "Variable",
        };
        self.err(format!("{role} names start with a lowercase letter"))
            .suggest(self.example_line(self.pos, &lowered, true))
    }

    pub(super) fn expect_word(&mut self, verb: &str, word: Kw) -> Result<(), Diagnostic> {
        if self.eat_kw(word) {
            return Ok(());
        }
        let ends_here = matches!(self.peek(), Some(Tok::Newline) | None);
        let word = word.as_str();
        if let Some(next) = ends_here.then(|| after_word(verb, word)).flatten() {
            return Err(self.err(format!("{verb} needs {word} and {next} after it")));
        }
        let example = self.fixed_line(word);
        Err(self.err(format!("{verb} needs {word}, as in {example}")))
    }

    /// The statement's tokens as one line, with `word` replacing the token at
    /// index `at` or inserted before it.
    pub(super) fn example_line(&self, at: usize, word: &str, replace: bool) -> String {
        let line_start = self.toks[..at]
            .iter()
            .rposition(|t| t.tok == Tok::Newline)
            .map_or(0, |i| i + 1);
        let layout = |t: &&Token| matches!(t.tok, Tok::Indent | Tok::Dedent);
        let start = line_start + self.toks[line_start..at].iter().take_while(layout).count();
        let end = self.toks[at..]
            .iter()
            .position(|t| t.tok == Tok::Newline)
            .map_or(self.toks.len(), |i| at + i);
        let mut words: Vec<String> = self.toks[start..end]
            .iter()
            .map(|t| t.tok.to_string())
            .collect();
        if replace && at - start < words.len() {
            words[at - start] = word.to_string();
        } else {
            words.insert(at - start, word.to_string());
        }
        join_words(&words)
    }

    /// The tokens from `start` to the end of the line, written as one line.
    pub(super) fn words_from(&self, start: usize) -> String {
        let words: Vec<String> = self.toks[start..]
            .iter()
            .take_while(|t| t.tok != Tok::Newline)
            .map(|t| t.tok.to_string())
            .collect();
        join_words(&words)
    }

    /// The statement with `word` where parsing stopped: replacing a wrong
    /// connecting word such as `into`, else inserted before the next token.
    pub(super) fn fixed_line(&self, word: &str) -> String {
        let wrong_word = matches!(
            self.peek(),
            Some(Tok::Kw(
                Kw::To
                    | Kw::From
                    | Kw::Into
                    | Kw::By
                    | Kw::As
                    | Kw::In
                    | Kw::Of
                    | Kw::With
                    | Kw::At
            ))
        );
        self.example_line(self.pos, word, wrong_word)
    }
}

/// What follows the connecting `word` of `verb`, for a statement that ends before it.
fn after_word(verb: &str, word: &str) -> Option<&'static str> {
    Some(match (verb, word) {
        ("set" | "multiply" | "divide" | "for each", _) => "a value",
        ("add" | "append" | "subtract" | "ask", _) => "a name",
        ("a field" | "needs" | "list", _) => "a type",
        ("table", "of") => "a record name",
        _ => return None,
    })
}

/// Joins tokens with single spaces, none after `(` or before `)` and `,`.
fn join_words(words: &[String]) -> String {
    let mut line = String::new();
    for (i, word) in words.iter().enumerate() {
        let after_open = i > 0 && words[i - 1] == "(";
        if i > 0 && !after_open && word != ")" && word != "," {
            line.push(' ');
        }
        line.push_str(word);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::ast::sexpr::stmt;
    use crate::lexer::{Tok, lex};
    use crate::parser::parse_stmts_str;
    use crate::source::Source;

    fn ss(t: &str) -> Vec<String> {
        parse_stmts_str(t).unwrap().iter().map(stmt).collect()
    }

    fn perr(t: &str) -> String {
        parse_stmts_str(t).unwrap_err().render()
    }

    #[test]
    fn simple_statements() {
        assert_eq!(
            ss(
                "set total to 0\nadd price to total\nsubtract d from total\nmultiply total by 2\ndivide total by people"
            ),
            [
                "(set total 0)",
                "(add price total)",
                "(subtract d total)",
                "(multiply total 2)",
                "(divide total people)"
            ]
        );
        assert_eq!(
            ss("set amount of item 3 of expenses to 150"),
            ["(set (of amount (item 3 expenses)) 150)"]
        );
        assert_eq!(
            ss(r#"append Expense with name "L", amount 1 to expenses"#),
            [r#"(append (record Expense (name "L") (amount 1)) expenses)"#]
        );
        assert_eq!(
            ss(r#"show "Total: ", (f with kind), total"#),
            [r#"(show "Total: " (paren (call f kind)) total)"#]
        );
        assert_eq!(
            ss(r#"ask "How much?" into cost"#),
            [r#"(ask "How much?" cost)"#]
        );
        assert_eq!(
            ss("skip\nstop\ngive total\nrun report with a, b\nrun start"),
            [
                "skip",
                "stop",
                "(give total)",
                "(run report a b)",
                "(run start)"
            ]
        );
        assert_eq!(
            ss("load expenses\nsave expenses\nclear expenses"),
            ["(load expenses)", "(save expenses)", "(clear expenses)"]
        );
        assert_eq!(
            ss(r#"expect total is 420
input "Lunch", "120"
note hi there"#),
            [
                "(expect (is total 420))",
                r#"(input "Lunch" "120")"#,
                r#"(note "hi there")"#
            ]
        );
    }

    #[test]
    fn example_line_replacing_past_the_end_inserts() {
        let src = Source::new("table t of E in");
        let toks = lex(&src).unwrap();
        let end = toks.iter().position(|t| t.tok == Tok::Newline);
        let at = end.unwrap_or(toks.len());
        let parser = Parser::new(&toks, &src);
        assert_eq!(parser.example_line(at, "x", true), "table t of E in x");
    }

    #[test]
    fn examples_use_canonical_spacing() {
        let error = parse_stmts_str("add (f with a, b) total").unwrap_err();
        assert_eq!(
            error.render(),
            "line 1: add (f with a, b) total. add needs to, as in add (f with a, b) to total."
        );
    }

    #[test]
    fn a_missing_word_at_the_end_has_no_dangling_example() {
        assert_eq!(
            perr("add 1"),
            "line 1: add 1. add needs to and a name after it."
        );
        assert_eq!(
            perr("set total"),
            "line 1: set total. set needs to and a value after it."
        );
        assert_eq!(
            perr("add 1 total"),
            "line 1: add 1 total. add needs to, as in add 1 to total."
        );
    }

    #[test]
    fn keyword_is_not_a_variable() {
        assert_eq!(
            perr("set add to 1"),
            "line 1: set add to 1. set needs a name after it."
        );
    }

    #[test]
    fn missing_words_and_names() {
        let rows = [
            ("add 1 into total", "add needs to, as in add 1 to total"),
            ("add 1 total", "add needs to, as in add 1 to total"),
            ("add x x", "add needs to, as in add x to x"),
            (
                "multiply total 2",
                "multiply needs by, as in multiply total by 2",
            ),
            ("add 1 to", "add needs a name after it"),
            ("subtract 5 from", "subtract needs a name after it"),
            ("show 1 2", "Unexpected 2 after the statement"),
        ];
        for (text, problem) in rows {
            assert_eq!(perr(text), format!("line 1: {text}. {problem}."));
        }
    }

    #[test]
    fn statement_errors() {
        assert_eq!(
            perr("set total as 0"),
            "line 1: set total as 0. set needs to. Did you mean set total to 0?"
        );
        assert_eq!(
            perr("total = 0"),
            "line 1: total = 0. = is not part of Gloss. Comparisons are written in words, such as is."
        );
        assert_eq!(
            perr("total to 0"),
            "line 1: total to 0. Every statement starts with a keyword, such as set or show; total is not one."
        );
        assert_eq!(
            perr("add 1 into total"),
            "line 1: add 1 into total. add needs to, as in add 1 to total."
        );
        assert_eq!(
            perr(r#"show "a", f with b"#),
            "line 1: show \"a\", f with b. f with ... must be in parentheses here, because it is inside arithmetic or a comma list."
        );
        assert_eq!(
            perr("run f with"),
            "line 1: run f with. with needs at least one input after it."
        );
        assert_eq!(
            perr("set Total to 1"),
            "line 1: set Total to 1. Variable names start with a lowercase letter. Did you mean set total to 1?"
        );
    }

    #[test]
    fn capital_names_are_lowercased_by_role() {
        assert_eq!(
            perr(r#"ask "What Name" into Name"#),
            r#"line 1: ask "What Name" into Name. Variable names start with a lowercase letter. Did you mean ask "What Name" into name?"#
        );
        assert_eq!(
            perr("run Report"),
            "line 1: run Report. Feature names start with a lowercase letter. Did you mean run report?"
        );
        assert_eq!(
            perr("save Expenses"),
            "line 1: save Expenses. Table and list names start with a lowercase letter. Did you mean save expenses?"
        );
    }
}
