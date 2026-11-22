use super::Parser;
use crate::ast::print;
use crate::ast::{CompareOp, Expr, ExprKind as K};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Sym, Tok};

impl Parser<'_> {
    /// An open form (call, record building, list building), else arithmetic.
    pub fn parse_value(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        match (self.peek().cloned(), self.peek_at(1)) {
            (Some(Tok::Name(feature)), Some(Tok::Kw(Kw::With))) => {
                self.pos += 2;
                let inputs = self.comma_list()?;
                Ok(self.mk(line, K::Call { feature, inputs }))
            }
            (Some(Tok::RecordName(record)), Some(Tok::Kw(Kw::With))) => {
                self.pos += 2;
                let fields = self.record_fields(&record)?;
                Ok(self.mk(line, K::BuildRecord { record, fields }))
            }
            (Some(Tok::Kw(Kw::List)), Some(Tok::Kw(Kw::Of))) => {
                self.pos += 2;
                let items = self.comma_list()?;
                Ok(self.mk(line, K::BuildList(items)))
            }
            _ => self.parse_arith(),
        }
    }

    fn comma_list(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut items = vec![self.parse_arith()?];
        while self.eat_sym(Sym::Comma) {
            items.push(self.parse_arith()?);
        }
        Ok(items)
    }

    fn record_fields(&mut self, record: &str) -> Result<Vec<(String, Expr)>, Diagnostic> {
        let mut fields = vec![self.record_field(record)?];
        while self.eat_sym(Sym::Comma) {
            fields.push(self.record_field(record)?);
        }
        Ok(fields)
    }

    fn record_field(&mut self, record: &str) -> Result<(String, Expr), Diagnostic> {
        let Some(Tok::Name(field)) = self.peek().cloned() else {
            return Err(self.err(format!(
                "{record} with needs a field name, as in {record} with name ..."
            )));
        };
        self.pos += 1;
        Ok((field, self.parse_arith()?))
    }

    /// Negations joined by `and` or by `or`, never both.
    pub fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        let first = self.negation()?;
        let Some(joiner) = self.joiner() else {
            return Ok(first);
        };
        let mut parts = vec![first];
        while self.eat_kw(joiner) {
            parts.push(self.negation()?);
        }
        if let Some(other) = self.joiner() {
            return Err(self.mixing_error(joiner, other, &parts));
        }
        let kind = if joiner == Kw::And {
            K::And(parts)
        } else {
            K::Or(parts)
        };
        Ok(self.mk(line, kind))
    }

    fn joiner(&self) -> Option<Kw> {
        match self.peek() {
            Some(Tok::Kw(k @ (Kw::And | Kw::Or))) => Some(*k),
            _ => None,
        }
    }

    /// Reads the parts after the other joiner, to show them in the message.
    fn mixing_error(&mut self, joiner: Kw, other: Kw, parts: &[Expr]) -> Diagnostic {
        self.pos += 1;
        let mut rest = Vec::new();
        loop {
            match self.negation() {
                Ok(next) => rest.push(next),
                Err(error) => return error,
            }
            if !self.eat_kw(other) {
                break;
            }
        }
        if self.joiner() == Some(joiner) {
            return self.err(
                "and and or cannot be mixed without parentheses; \
                 put parentheses around the parts that belong together",
            );
        }
        let (word, other) = (joiner.as_str(), other.as_str());
        let (last, before) = parts.split_last().expect("at least two parts");
        let rest = join_parts(&rest, other);
        let (all, head) = (join_parts(parts, word), join_parts(before, word));
        let tail = format!("({} {other} {rest})", print::expr(last));
        self.err(format!(
            "and and or cannot be mixed without parentheses. \
             Write ({all}) {other} {rest}, or {head} {word} {tail}"
        ))
    }

    fn negation(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        if !self.eat_kw(Kw::Not) {
            return self.comparison();
        }
        let operand = self.comparison()?;
        Ok(self.mk(line, K::Not(Box::new(operand))))
    }

    fn comparison(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_value()?;
        if !self.eat_kw(Kw::Is) {
            return Ok(left);
        }
        let line = left.line;
        let left = Box::new(left);
        let kind = if self.eat_kw(Kw::Number) {
            K::IsNumber {
                value: left,
                negated: false,
            }
        } else if self.peek() == Some(&Tok::Kw(Kw::Not))
            && self.peek_at(1) == Some(&Tok::Kw(Kw::Number))
        {
            self.pos += 2;
            K::IsNumber {
                value: left,
                negated: true,
            }
        } else {
            let op = self.compare_op(&left)?;
            let right = Box::new(self.parse_value()?);
            K::Compare { op, left, right }
        };
        if self.peek() == Some(&Tok::Kw(Kw::Is)) {
            return Err(self.err("Comparisons cannot be chained; join two comparisons with and"));
        }
        Ok(self.mk(line, kind))
    }

    /// The word or words after `is`, up to the right-hand value.
    fn compare_op(&mut self, left: &Expr) -> Result<CompareOp, Diagnostic> {
        if self.eat_kw(Kw::Not) {
            return Ok(CompareOp::IsNot);
        }
        if self.eat_kw(Kw::Greater) {
            self.expect_than(left, "greater")?;
            return Ok(CompareOp::Greater);
        }
        if self.eat_kw(Kw::Less) {
            self.expect_than(left, "less")?;
            return Ok(CompareOp::Less);
        }
        if !self.eat_kw(Kw::At) {
            return Ok(CompareOp::Is);
        }
        if self.eat_kw(Kw::Least) {
            Ok(CompareOp::AtLeast)
        } else if self.eat_kw(Kw::Most) {
            Ok(CompareOp::AtMost)
        } else {
            Err(self.err("is at needs least or most"))
        }
    }

    fn expect_than(&mut self, left: &Expr, word: &str) -> Result<(), Diagnostic> {
        if self.eat_kw(Kw::Than) {
            return Ok(());
        }
        let right = self.parse_value()?;
        Err(self.err(format!(
            "is {word} needs than, as in {} is {word} than {}",
            print::expr(left),
            print::expr(&right)
        )))
    }
}

fn join_parts(parts: &[Expr], word: &str) -> String {
    let shown: Vec<String> = parts.iter().map(print::expr).collect();
    shown.join(&format!(" {word} "))
}

#[cfg(test)]
mod tests {
    use crate::ast::sexpr::sexpr;
    use crate::parser::parse_expr_str;

    fn sx(t: &str) -> String {
        sexpr(&parse_expr_str(t).unwrap())
    }

    fn perr(t: &str) -> String {
        parse_expr_str(t).unwrap_err().problem
    }

    #[test]
    fn open_forms() {
        assert_eq!(sx("f with a, b"), "(call f a b)");
        assert_eq!(sx("f with a + 1"), "(call f (+ a 1))");
        assert_eq!(sx("is_even with n"), "(call is_even n)");
        assert_eq!(
            sx(r#"Expense with name "L", amount 120"#),
            r#"(record Expense (name "L") (amount 120))"#
        );
        assert_eq!(sx("list of 4, 9, 2"), "(list 4 9 2)");
        assert_eq!(
            sx(r#"list of (S with n "A", m 1), (S with n "B", m 2)"#),
            r#"(list (paren (record S (n "A") (m 1))) (paren (record S (n "B") (m 2))))"#
        );
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(
            sx(r#"category_total with "food" is 420"#),
            r#"(is (call category_total "food") 420)"#
        );
        assert_eq!(sx("f with (x is 3)"), "(call f (paren (is x 3)))");
        assert_eq!(sx("cost is not number"), "(is-not-number cost)");
        assert_eq!(sx("cost is number"), "(is-number cost)");
        assert_eq!(sx("a is not b"), "(is-not a b)");
        assert_eq!(sx("a is greater than b"), "(greater a b)");
        assert_eq!(sx("a is less than b"), "(less a b)");
        assert_eq!(sx("a is at least 3"), "(at-least a 3)");
        assert_eq!(sx("a is at most 3"), "(at-most a 3)");
        assert_eq!(sx("remainder of n by 2 is 0"), "(is (remainder n 2) 0)");
        assert_eq!(sx("not a and b"), "(and (not a) b)");
        assert_eq!(sx("a or b or c"), "(or a b c)");
        assert_eq!(sx("(a and b) or c"), "(or (paren (and a b)) c)");
    }

    #[test]
    fn logic_errors() {
        assert_eq!(
            perr("a is b is c"),
            "Comparisons cannot be chained; join two comparisons with and"
        );
        assert_eq!(
            perr("a and b or c"),
            "and and or cannot be mixed without parentheses. \
             Write (a and b) or c, or a and (b or c)"
        );
        assert_eq!(
            perr("a is greater b"),
            "is greater needs than, as in a is greater than b"
        );
        assert_eq!(
            perr("a and b or c or d"),
            "and and or cannot be mixed without parentheses. \
             Write (a and b) or c or d, or a and (b or c or d)"
        );
        assert_eq!(
            perr("a and b or c and d"),
            "and and or cannot be mixed without parentheses; \
             put parentheses around the parts that belong together"
        );
        assert_eq!(
            perr("a or b and c"),
            "and and or cannot be mixed without parentheses. \
             Write (a or b) and c, or a or (b and c)"
        );
        assert_eq!(
            perr("x is greater y + 1"),
            "is greater needs than, as in x is greater than y + 1"
        );
        assert_eq!(
            perr("x is less y + 1"),
            "is less needs than, as in x is less than y + 1"
        );
        assert_eq!(perr("a is at 3"), "is at needs least or most");
        assert_eq!(
            perr("1 + f with a"),
            "f with ... must be in parentheses here, \
             because it is inside arithmetic or a comma list"
        );
    }

    #[test]
    fn open_form_errors() {
        let open = "f with ... must be in parentheses here, \
                    because it is inside arithmetic or a comma list";
        assert_eq!(perr("list of f with a"), open);
        assert_eq!(
            perr("Expense with 3"),
            "Expense with needs a field name, as in Expense with name ..."
        );
    }
}
