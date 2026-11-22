use super::Parser;
use crate::ast::{Expr, ExprKind as K, ItemType, PlainType};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Sym, Tok};

impl Parser<'_> {
    pub fn parse_arith(&mut self) -> Result<Expr, Diagnostic> {
        // Later commits add the arithmetic levels above this one.
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        let kind = match self.bump().map(|t| t.tok) {
            Some(Tok::Number(n)) => K::Number(n),
            Some(Tok::Text(t)) => K::Text(t),
            Some(Tok::Kw(Kw::Yes)) => K::Truth(true),
            Some(Tok::Kw(Kw::No)) => K::Truth(false),
            Some(Tok::Kw(Kw::Shown)) => K::Shown,
            Some(Tok::Name(name)) => K::Name(name),
            Some(Tok::Kw(Kw::Empty)) => self.empty_list()?,
            Some(Tok::Sym(Sym::LParen)) => self.parenthesized()?,
            Some(other) => return Err(self.err(format!("Expected a value but found {other}"))),
            None => return Err(self.err("Expected a value")),
        };
        Ok(self.mk(line, kind))
    }

    fn empty_list(&mut self) -> Result<K, Diagnostic> {
        if !(self.eat_kw(Kw::List) && self.eat_kw(Kw::Of)) {
            return Err(self.err("empty must be followed by list of, as in empty list of number"));
        }
        Ok(K::EmptyList(self.parse_item_type()?))
    }

    fn parenthesized(&mut self) -> Result<K, Diagnostic> {
        // Task 8 parses a full expression here.
        let inner = self.parse_arith()?;
        if !self.eat_sym(Sym::RParen) {
            return Err(self.err("Expected )"));
        }
        Ok(K::Paren(Box::new(inner)))
    }

    pub fn parse_item_type(&mut self) -> Result<ItemType, Diagnostic> {
        match self.bump().map(|t| t.tok) {
            Some(Tok::Kw(Kw::Number)) => Ok(ItemType::Plain(PlainType::Number)),
            Some(Tok::Kw(Kw::Text)) => Ok(ItemType::Plain(PlainType::Text)),
            Some(Tok::Kw(Kw::Truth)) => Ok(ItemType::Plain(PlainType::Truth)),
            Some(Tok::RecordName(name)) => Ok(ItemType::Record(name)),
            _ => Err(self.err("Expected number, text, truth or a record name")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::sexpr::sexpr;
    use crate::parser::parse_expr_str;

    fn sx(t: &str) -> String {
        sexpr(&parse_expr_str(t).unwrap())
    }

    #[test]
    fn primary_values() {
        assert_eq!(sx("7.5"), "7.5");
        assert_eq!(sx("\"hi\""), "\"hi\"");
        assert_eq!(sx("total"), "total");
        assert_eq!(sx("(total)"), "(paren total)");
        assert_eq!(sx("empty list of Student"), "(empty-list Student)");
        assert_eq!(sx("empty list of number"), "(empty-list number)");
        assert_eq!(sx("empty list of truth"), "(empty-list truth)");
        assert_eq!(sx("shown"), "shown");
        assert_eq!(sx("yes"), "yes");
        assert_eq!(sx("no"), "no");
    }
}
