use super::Parser;
use crate::ast::print;
use crate::ast::{ArithOp, Conversion, Expr, ExprKind as K, Fact, ItemType, PlainType};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Sym, Tok};

impl Parser<'_> {
    pub fn parse_arith(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.product()?;
        while let Some(op) = self.arith_op(Sym::Plus, Sym::Minus, ArithOp::Add, ArithOp::Sub) {
            let right = self.right_operand(op, &left, Self::product)?;
            left = self.binary(op, left, right);
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.converted()?;
        while let Some(op) = self.arith_op(Sym::Star, Sym::Slash, ArithOp::Mul, ArithOp::Div) {
            let right = self.right_operand(op, &left, Self::converted)?;
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

    /// Parses the operand after `op`, rejecting a `whole` or `remainder`
    /// fact on the right edge of `left`, which could be read two ways.
    fn right_operand(
        &mut self,
        op: ArithOp,
        left: &Expr,
        level: fn(&mut Self) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        if matches!(self.peek(), Some(Tok::Newline) | None) {
            return Err(self.err(format!("{} needs a value after it", op_text(op))));
        }
        let right = level(self)?;
        match number_fact_on_right_edge(left) {
            Some(fact) => {
                let message = ambiguity(&print::expr(left), fact, op_text(op), &right);
                Err(Diagnostic::error(self.src, fact.line, message))
            }
            None => Ok(right),
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
            let of = Box::new(self.nested(Self::fact)?);
            return Ok(self.mk(line, K::Fact { fact, of }));
        }
        let kind = match self.peek() {
            Some(Tok::Name(name)) if followed_by_of => {
                let field = name.clone();
                self.pos += 2;
                K::FieldOf {
                    field,
                    of: Box::new(self.nested(Self::fact)?),
                }
            }
            Some(Tok::Kw(k)) if built_in_fact(*k).is_some() => {
                let word = k.as_str();
                return Err(self.err(format!("{word} needs of, as in {word} of prices")));
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
        let start = self.pos;
        let position = self.position()?;
        if !self.eat_kw(Kw::Of) {
            let example = self.item_example(start);
            return Err(self.err(format!(
                "item needs of after its position; a position that is worked out \
                 goes in parentheses, as in {example}"
            )));
        }
        Ok(K::ItemOf {
            position: Box::new(position),
            of: Box::new(self.nested(Self::fact)?),
        })
    }

    /// Rewrites the tokens of a worked-out position as `item (...) of x`.
    fn item_example(&self, start: usize) -> String {
        let rest = &self.toks[start..];
        let line_end = rest.iter().position(|t| t.tok == Tok::Newline);
        let statement = &rest[..line_end.unwrap_or(rest.len())];
        let end = statement.iter().position(|t| t.tok == Tok::Kw(Kw::Of));
        let position: Vec<String> = statement[..end.unwrap_or(statement.len())]
            .iter()
            .map(|t| t.tok.to_string())
            .collect();
        let target = end
            .and_then(|i| statement.get(i + 1))
            .map_or("prices".into(), |t| t.tok.to_string());
        format!("item ({}) of {target}", position.join(" "))
    }

    pub(super) fn position(&mut self) -> Result<Expr, Diagnostic> {
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
            Some(Tok::Name(name)) => self.reject_open_form(&name).map(|()| K::Name(name))?,
            Some(Tok::RecordName(name)) => return Err(self.record_value_error(&name)),
            Some(Tok::Kw(Kw::Empty)) => self.empty_list()?,
            Some(Tok::Sym(Sym::LParen)) => self.parenthesized()?,
            Some(other) => return Err(self.err(format!("Expected a value but found {other}"))),
            None => return Err(self.err("Expected a value")),
        };
        Ok(self.mk(line, kind))
    }

    fn reject_open_form(&self, name: &str) -> Result<(), Diagnostic> {
        if self.peek() != Some(&Tok::Kw(Kw::With)) {
            return Ok(());
        }
        Err(self.err(format!(
            "{name} with ... must be in parentheses here, \
             because it is inside arithmetic or a comma list"
        )))
    }

    fn record_value_error(&self, name: &str) -> Diagnostic {
        match self.reject_open_form(name) {
            Err(error) => error,
            Ok(()) => self.err(format!(
                "{name} is a record; build one with {name} with ..."
            )),
        }
    }

    fn empty_list(&mut self) -> Result<K, Diagnostic> {
        if !(self.eat_kw(Kw::List) && self.eat_kw(Kw::Of)) {
            return Err(self.err("empty must be followed by list of, as in empty list of number"));
        }
        Ok(K::EmptyList(self.parse_item_type()?))
    }

    fn parenthesized(&mut self) -> Result<K, Diagnostic> {
        let inner = self.nested(Self::parse_expr)?;
        if !self.eat_sym(Sym::RParen) {
            return Err(self.err("This ( has no matching )"));
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

fn op_text(op: ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    }
}

/// The `whole` or `remainder` fact that ends `e`, looking through
/// negation and the right side of arithmetic.
fn number_fact_on_right_edge(e: &Expr) -> Option<&Expr> {
    match &e.kind {
        K::Fact {
            fact: Fact::Whole, ..
        }
        | K::Remainder { .. } => Some(e),
        K::Negate(inner) => number_fact_on_right_edge(inner),
        K::Binary { right, .. } => number_fact_on_right_edge(right),
        _ => None,
    }
}

/// The message for `left op right`, where `fact` ends `left`; both ways of reading it are
/// written out with everything that comes before the fact kept in place.
fn ambiguity(left: &str, fact: &Expr, op: &str, right: &Expr) -> String {
    let (f, r) = (print::expr(fact), print::expr(right));
    let before = left.strip_suffix(f.as_str()).unwrap_or_default();
    let second = match &fact.kind {
        K::Fact { of, .. } => format!("whole of ({} {op} {r})", print::expr(of)),
        K::Remainder { of, by } => {
            format!(
                "remainder of {} by ({} {op} {r})",
                print::expr(of),
                print::expr(by)
            )
        }
        _ => String::new(),
    };
    format!("{f} {op} {r} can be read two ways. Write {before}({f}) {op} {r} or {before}{second}")
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

    fn perr(t: &str) -> String {
        parse_expr_str(t).unwrap_err().problem
    }

    #[test]
    fn ambiguity_and_errors() {
        assert_eq!(
            perr("whole of a / b"),
            "whole of a / b can be read two ways. Write (whole of a) / b or whole of (a / b)"
        );
        assert_eq!(
            perr("remainder of a by b + 1"),
            "remainder of a by b + 1 can be read two ways. Write (remainder of a by b) + 1 or remainder of a by (b + 1)"
        );
        assert_eq!(
            perr("2 * whole of x + 1"),
            "whole of x + 1 can be read two ways. Write 2 * (whole of x) + 1 or 2 * whole of (x + 1)"
        );
        assert_eq!(
            perr("-whole of a / b"),
            "whole of a / b can be read two ways. Write -(whole of a) / b or -whole of (a / b)"
        );
        assert_eq!(
            perr("a - whole of x + 1"),
            "whole of x + 1 can be read two ways. Write a - (whole of x) + 1 or a - whole of (x + 1)"
        );
        assert_eq!(
            perr("item i + 1 of prices"),
            "item needs of after its position; a position that is worked out goes in parentheses, as in item (i + 1) of prices"
        );
        assert_eq!(
            perr("total as text with 5 places"),
            "The number of places is a digit from 0 to 4"
        );
        assert_eq!(
            perr("count prices"),
            "count needs of, as in count of prices"
        );
        assert_eq!(perr("1 +"), "+ needs a value after it");
        assert_eq!(perr("(1 + 2"), "This ( has no matching )");
        assert_eq!(perr("1 2"), "Unexpected 2 after the expression");
    }

    #[test]
    fn item_example_stays_on_the_statement() {
        assert_eq!(
            perr("item i + 1\nof prices"),
            "item needs of after its position; a position that is worked out goes in parentheses, as in item (i + 1) of prices"
        );
        assert_eq!(
            perr("item i + 1\nof total"),
            "item needs of after its position; a position that is worked out goes in parentheses, as in item (i + 1) of prices"
        );
    }

    #[test]
    fn deep_nesting_is_reported_not_a_crash() {
        // The 10 000-level inputs are rejected long before the limit's stack use matters, but
        // a big stack keeps this test independent of how large debug frames get.
        let run = || {
            let parens = "(".repeat(10_000);
            assert_eq!(perr(&parens), "This expression is nested too deeply");
            let facts = "count of ".repeat(10_000);
            assert_eq!(perr(&facts), "This expression is nested too deeply");
            let ok = format!("{}1{}", "(".repeat(100), ")".repeat(100));
            assert!(parse_expr_str(&ok).is_ok());
        };
        let big = std::thread::Builder::new().stack_size(64 << 20);
        big.spawn(run).unwrap().join().unwrap();
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
