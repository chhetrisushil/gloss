use super::Parser;
use crate::ast::{Expr, ExprKind as K};
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
