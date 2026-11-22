use super::Parser;
use crate::ast::stmt::{Stmt, StmtKind as S};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Tok};

impl Parser<'_> {
    /// Reads a block statement; the current token is its keyword.
    pub(super) fn block_stmt(&mut self) -> Result<S, Diagnostic> {
        let word = self.bump().map(|t| t.tok);
        match word {
            Some(Tok::Kw(Kw::If)) => self.if_stmt(),
            Some(Tok::Kw(Kw::For)) => self.for_stmt(),
            Some(Tok::Kw(Kw::Repeat)) => {
                if !self.eat_kw(Kw::While) {
                    return Err(self.repeat_needs_while());
                }
                let cond = self.parse_expr()?;
                let body = self.parse_block("repeat")?;
                Ok(S::RepeatWhile { cond, body })
            }
            Some(Tok::Kw(Kw::Otherwise)) => Err(self.err("otherwise must follow an if block")),
            _ => Err(self.err("Expected a block statement")),
        }
    }

    /// Reads a newline, an indent, then statements up to the matching dedent.
    pub fn parse_block(&mut self, owner: &str) -> Result<Vec<Stmt>, Diagnostic> {
        if let Some(found) = self.peek().filter(|t| **t != Tok::Newline) {
            return Err(self.err(format!("Unexpected {found} after the {owner} line")));
        }
        let indented = self.peek() == Some(&Tok::Newline) && self.peek_at(1) == Some(&Tok::Indent);
        if !indented {
            return Err(self.err(format!("{owner} needs an indented block under it")));
        }
        self.pos += 2;
        let mut body = Vec::new();
        while !matches!(self.peek(), Some(Tok::Dedent) | None) {
            body.push(self.parse_stmt()?);
        }
        if self.peek() == Some(&Tok::Dedent) {
            self.pos += 1;
        }
        Ok(body)
    }

    fn if_stmt(&mut self) -> Result<S, Diagnostic> {
        let cond = self.parse_expr()?;
        let mut branches = vec![(cond, self.parse_block("if")?)];
        let mut otherwise = None;
        while self.eat_kw(Kw::Otherwise) {
            if self.eat_kw(Kw::If) {
                let cond = self.parse_expr()?;
                branches.push((cond, self.parse_block("otherwise if")?));
            } else {
                otherwise = Some(self.parse_block("otherwise")?);
                break;
            }
        }
        Ok(S::If {
            branches,
            otherwise,
        })
    }

    fn for_stmt(&mut self) -> Result<S, Diagnostic> {
        if !self.eat_kw(Kw::Each) {
            let example = self.example_line(self.pos, "each", false);
            return Err(self.err(format!("for needs each, as in {example}")));
        }
        let var = self.parse_name("for each")?;
        if self.eat_kw(Kw::In) {
            let list = self.parse_arith()?;
            let body = self.parse_block("for each")?;
            return Ok(S::ForEachIn { var, list, body });
        }
        if self.eat_kw(Kw::From) {
            let from = self.parse_arith()?;
            self.expect_word("for each", Kw::To)?;
            let to = self.parse_arith()?;
            let body = self.parse_block("for each")?;
            return Ok(S::ForEachRange {
                var,
                from,
                to,
                body,
            });
        }
        let example = self.fixed_line("in");
        Err(self.err(format!("for each needs in or from, as in {example}")))
    }

    /// Any single word where `while` belongs is replaced by it.
    fn repeat_needs_while(&self) -> Diagnostic {
        let at_end = matches!(self.peek(), Some(Tok::Newline) | None);
        let example = self.example_line(self.pos, "while", !at_end);
        self.err(format!("repeat needs while, as in {example}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::sexpr::stmt;
    use crate::parser::parse_stmts_str;

    fn ss(t: &str) -> Vec<String> {
        parse_stmts_str(t).unwrap().iter().map(stmt).collect()
    }

    fn perr(t: &str) -> String {
        parse_stmts_str(t).unwrap_err().render()
    }

    #[test]
    fn blocks() {
        let p = "if a is 1\n  show 1\notherwise if a is 2\n  show 2\notherwise\n  show 3\n  stop";
        assert_eq!(
            ss(p),
            ["(if (is a 1) [(show 1)] (is a 2) [(show 2)] else [(show 3) stop])"]
        );
        assert_eq!(
            ss("for each price in prices\n  add price to total"),
            ["(for-each price prices [(add price total)])"]
        );
        assert_eq!(
            ss("for each i from 1 to count of xs\n  skip"),
            ["(for-range i 1 (count xs) [skip])"]
        );
        assert_eq!(
            ss("repeat while cost is not number\n  ask \"Again\" into cost"),
            [r#"(repeat (is-not-number cost) [(ask "Again" cost)])"#]
        );
        assert_eq!(
            ss("if a\n  if b\n    stop\nshow 1"),
            ["(if a [(if b [stop])])", "(show 1)"]
        );
    }

    #[test]
    fn block_errors() {
        assert_eq!(
            perr("if a\nshow 1"),
            "line 1: if a. if needs an indented block under it."
        );
        assert_eq!(
            perr("otherwise\n  show 1"),
            "line 1: otherwise. otherwise must follow an if block."
        );
        assert_eq!(
            perr("for each x of xs\n  stop"),
            "line 1: for each x of xs. for each needs in or from, as in for each x in xs."
        );
        for (name, list) in [("x", "xs"), ("price", "prices")] {
            let line = format!("for {name} in {list}");
            assert_eq!(
                perr(&format!("{line}\n  stop")),
                format!("line 1: {line}. for needs each, as in for each {name} in {list}.")
            );
        }
        assert_eq!(
            ss("if a\n  if b\n    stop\n  otherwise\n    show 1"),
            ["(if a [(if b [stop] else [(show 1)])])"]
        );
        assert_eq!(
            perr("for each i from 1 3\n  stop"),
            "line 1: for each i from 1 3. for each needs to, as in for each i from 1 to 3."
        );
        assert_eq!(
            perr("for each x in a is b\n  stop"),
            "line 1: for each x in a is b. Unexpected is after the for each line."
        );
        assert_eq!(
            perr("if a b\n  stop"),
            "line 1: if a b. Unexpected b after the if line."
        );
        assert_eq!(
            perr("repeat until a\n  stop"),
            "line 1: repeat until a. repeat needs while, as in repeat while a."
        );
    }
}
