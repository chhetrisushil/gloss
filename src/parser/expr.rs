use super::Parser;
use crate::ast::{ArithOp, Conversion, Expr, ExprKind as K, Fact, ItemType, PlainType};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Sym, Tok};

impl Parser<'_> {
    pub fn parse_arith(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.product()?;
        while let Some(op) = self.arith_op(Sym::Plus, Sym::Minus, ArithOp::Add, ArithOp::Sub) {
            let right = self.product()?;
            left = self.binary(op, left, right);
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.converted()?;
        while let Some(op) = self.arith_op(Sym::Star, Sym::Slash, ArithOp::Mul, ArithOp::Div) {
            let right = self.converted()?;
            left = self.binary(op, left, right);
        }
        Ok(left)
    }

    /// Consumes one of two operator symbols, if it is next.
    fn arith_op(&mut self, a: Sym, b: Sym, op_a: ArithOp, op_b: ArithOp) -> Option<ArithOp> {
        if self.eat_sym(a) {
            Some(op_a)
        } else if self.eat_sym(b) {
            Some(op_b)
        } else {
            None
        }
    }

    fn binary(&mut self, op: ArithOp, left: Expr, right: Expr) -> Expr {
        let line = left.line;
        let kind = K::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        };
        self.mk(line, kind)
    }

    fn converted(&mut self) -> Result<Expr, Diagnostic> {
        let value = self.signed()?;
        if !self.eat_kw(Kw::As) {
            return Ok(value);
        }
        let to = self.conversion()?;
        let line = value.line;
        Ok(self.mk(
            line,
            K::Convert {
                value: Box::new(value),
                to,
            },
        ))
    }

    fn conversion(&mut self) -> Result<Conversion, Diagnostic> {
        if self.eat_kw(Kw::Number) {
            return Ok(Conversion::ToNumber);
        }
        if !self.eat_kw(Kw::Text) {
            return Err(self.err("as must be followed by number or text"));
        }
        if !self.eat_kw(Kw::With) {
            return Ok(Conversion::ToText);
        }
        let places = match self.bump().map(|t| t.tok) {
            Some(Tok::Number(n)) => n.show().parse::<u8>().ok().filter(|d| *d <= 4),
            _ => None,
        };
        let places =
            places.ok_or_else(|| self.err("The number of places is a digit from 0 to 4"))?;
        if !self.eat_kw(Kw::Places) {
            return Err(self.err("with needs places after the digit"));
        }
        Ok(Conversion::ToTextPlaces(places))
    }

    fn signed(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        if !self.eat_sym(Sym::Minus) {
            return self.fact();
        }
        let operand = self.fact()?;
        Ok(self.mk(line, K::Negate(Box::new(operand))))
    }

    fn fact(&mut self) -> Result<Expr, Diagnostic> {
        let line = self.line();
        let followed_by_of = self.peek_at(1) == Some(&Tok::Kw(Kw::Of));
        if let Some(fact) = self.built_in_fact_next() {
            self.pos += 2;
            let of = Box::new(self.fact()?);
            return Ok(self.mk(line, K::Fact { fact, of }));
        }
        let kind = match self.peek() {
            Some(Tok::Name(name)) if followed_by_of => {
                let field = name.clone();
                self.pos += 2;
                K::FieldOf {
                    field,
                    of: Box::new(self.fact()?),
                }
            }
            Some(Tok::Kw(Kw::Item)) => self.item_of()?,
            Some(Tok::Kw(Kw::Remainder)) => self.remainder()?,
            _ => return self.primary(),
        };
        Ok(self.mk(line, kind))
    }

    /// The built-in fact word that is next and followed by `of`, if any.
    fn built_in_fact_next(&self) -> Option<Fact> {
        match (self.peek(), self.peek_at(1)) {
            (Some(Tok::Kw(k)), Some(Tok::Kw(Kw::Of))) => built_in_fact(*k),
            _ => None,
        }
    }

    fn item_of(&mut self) -> Result<K, Diagnostic> {
        self.pos += 1;
        let position = self.position()?;
        if !self.eat_kw(Kw::Of) {
            return Err(self.err("item needs of after its position"));
        }
        Ok(K::ItemOf {
            position: Box::new(position),
            of: Box::new(self.fact()?),
        })
    }

    fn position(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek() {
            Some(Tok::Number(_) | Tok::Name(_) | Tok::Sym(Sym::LParen)) => self.primary(),
            _ => Err(self.err("item needs a position after it")),
        }
    }

    fn remainder(&mut self) -> Result<K, Diagnostic> {
        self.pos += 1;
        if !self.eat_kw(Kw::Of) {
            return Err(self.err("remainder needs of, as in remainder of n by 2"));
        }
        let of = self.fact()?;
        if !self.eat_kw(Kw::By) {
            return Err(self.err("remainder needs by after its first value"));
        }
        Ok(K::Remainder {
            of: Box::new(of),
            by: Box::new(self.fact()?),
        })
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

fn built_in_fact(k: Kw) -> Option<Fact> {
    match k {
        Kw::Count => Some(Fact::Count),
        Kw::First => Some(Fact::First),
        Kw::Last => Some(Fact::Last),
        Kw::Whole => Some(Fact::Whole),
        _ => None,
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

    #[test]
    fn precedence_and_facts() {
        assert_eq!(sx("1 + 2 * 3"), "(+ 1 (* 2 3))");
        assert_eq!(sx("a - b - c"), "(- (- a b) c)");
        assert_eq!(sx("count of prices + 1"), "(+ (count prices) 1)");
        assert_eq!(
            sx("name of item 2 of students"),
            "(of name (item 2 students))"
        );
        assert_eq!(
            sx("item (i + 1) of prices"),
            "(item (paren (+ i 1)) prices)"
        );
        assert_eq!(sx("item i of prices"), "(item i prices)");
        assert_eq!(sx("-amount of x"), "(- (of amount x))");
        assert_eq!(sx("-7.5"), "(- 7.5)");
        assert_eq!(sx("cost as number"), "(as-number cost)");
        assert_eq!(sx("total as text with 2 places"), "(as-text-2 total)");
        assert_eq!(sx("remainder of n by 2"), "(remainder n 2)");
        assert_eq!(sx("2 * whole of x"), "(* 2 (whole x))");
        assert_eq!(sx("(whole of a) / b"), "(/ (paren (whole a)) b)");
        assert_eq!(sx("whole of (a / b)"), "(whole (paren (/ a b)))");
    }
}
