use crate::ast::{ArithOp, CompareOp, Conversion, Expr, ExprKind as K, Fact, item_type_text};

pub fn expr(e: &Expr) -> String {
    match &e.kind {
        K::Number(n) => n.show(),
        K::Text(t) => format!("\"{}\"", t.replace('"', "\"\"")),
        K::Truth(b) => if *b { "yes" } else { "no" }.into(),
        K::Name(n) => n.clone(),
        K::Shown => "shown".into(),
        K::EmptyList(t) => format!("empty list of {}", item_type_text(t)),
        K::Paren(x) => format!("({})", expr(x)),
        K::FieldOf { field, of } => format!("{field} of {}", expr(of)),
        K::Fact { fact, of } => format!("{} of {}", fact_word(*fact), expr(of)),
        K::ItemOf { position, of } => format!("item {} of {}", expr(position), expr(of)),
        K::Remainder { of, by } => format!("remainder of {} by {}", expr(of), expr(by)),
        K::Negate(x) => format!("-{}", expr(x)),
        K::Convert { value, to } => format!("{} as {}", expr(value), conversion_text(*to)),
        K::Binary { op, left, right } => {
            format!("{} {} {}", expr(left), symbol(*op), expr(right))
        }
        K::Call { feature, inputs } if inputs.is_empty() => feature.clone(),
        K::Call { feature, inputs } => format!("{feature} with {}", join(inputs, ", ")),
        K::BuildRecord { record, fields } if fields.is_empty() => record.clone(),
        K::BuildRecord { record, fields } => {
            format!("{record} with {}", pairs(fields))
        }
        K::BuildList(xs) => format!("list of {}", join(xs, ", ")),
        K::Compare { op, left, right } => {
            format!("{} {} {}", expr(left), words(*op), expr(right))
        }
        K::IsNumber { value, negated } => {
            let not = if *negated { "not " } else { "" };
            format!("{} is {not}number", expr(value))
        }
        K::Not(x) => format!("not {}", expr(x)),
        K::And(xs) => join(xs, " and "),
        K::Or(xs) => join(xs, " or "),
    }
}

fn join(xs: &[Expr], sep: &str) -> String {
    xs.iter().map(expr).collect::<Vec<_>>().join(sep)
}

fn pairs(fields: &[(String, Expr)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{name} {}", expr(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn fact_word(f: Fact) -> &'static str {
    match f {
        Fact::Count => "count",
        Fact::First => "first",
        Fact::Last => "last",
        Fact::Whole => "whole",
    }
}

fn conversion_text(c: Conversion) -> String {
    match c {
        Conversion::ToNumber => "number".into(),
        Conversion::ToText => "text".into(),
        Conversion::ToTextPlaces(n) => format!("text with {n} places"),
    }
}

fn symbol(op: ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    }
}

fn words(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Is => "is",
        CompareOp::IsNot => "is not",
        CompareOp::Greater => "is greater than",
        CompareOp::Less => "is less than",
        CompareOp::AtLeast => "is at least",
        CompareOp::AtMost => "is at most",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArithOp, Conversion, ExprId, ExprKind as K, Fact, ItemType};
    use crate::number::Number;

    fn e(kind: K) -> Expr {
        Expr {
            id: ExprId(0),
            line: 1,
            kind,
        }
    }
    fn name(s: &str) -> Expr {
        e(K::Name(s.into()))
    }
    fn num(v: i64) -> Expr {
        e(K::Number(Number(v)))
    }
    fn bx(x: Expr) -> Box<Expr> {
        Box::new(x)
    }
    fn bin(op: ArithOp, l: Expr, r: Expr) -> Expr {
        e(K::Binary {
            op,
            left: bx(l),
            right: bx(r),
        })
    }
    fn cmp(op: CompareOp, l: Expr, r: Expr) -> Expr {
        e(K::Compare {
            op,
            left: bx(l),
            right: bx(r),
        })
    }
    fn check(x: Expr, want: &str) {
        assert_eq!(expr(&x), want, "{x:?}");
    }
    fn field_of(field: &str, of: Expr) -> Expr {
        e(K::FieldOf {
            field: field.into(),
            of: bx(of),
        })
    }
    fn fact(fact: Fact, of: Expr) -> Expr {
        e(K::Fact { fact, of: bx(of) })
    }

    #[test]
    fn prints_leaves_and_wrappers() {
        let student = ItemType::Record("Student".into());
        check(num(1_205_000), "120.5");
        check(e(K::Text("say \"hi\"".into())), "\"say \"\"hi\"\"\"");
        for (b, want) in [(true, "yes"), (false, "no")] {
            check(e(K::Truth(b)), want);
        }
        check(e(K::Shown), "shown");
        check(e(K::EmptyList(student)), "empty list of Student");
        check(e(K::Paren(bx(name("a")))), "(a)");
        check(e(K::Negate(bx(name("x")))), "-x");
    }

    #[test]
    fn prints_accessors_conversions_and_arithmetic() {
        check(field_of("amount", name("x")), "amount of x");
        let facts = [
            (Fact::Count, "count of xs"),
            (Fact::First, "first of xs"),
            (Fact::Last, "last of xs"),
            (Fact::Whole, "whole of xs"),
        ];
        for (f, want) in facts {
            check(fact(f, name("xs")), want);
        }
        check(
            e(K::ItemOf {
                position: bx(num(20_000)),
                of: bx(name("xs")),
            }),
            "item 2 of xs",
        );
        check(
            e(K::Remainder {
                of: bx(name("n")),
                by: bx(num(20_000)),
            }),
            "remainder of n by 2",
        );
        let conversions = [
            (Conversion::ToNumber, "x as number"),
            (Conversion::ToText, "x as text"),
            (Conversion::ToTextPlaces(2), "x as text with 2 places"),
        ];
        for (to, want) in conversions {
            let value = bx(name("x"));
            check(e(K::Convert { value, to }), want);
        }
        let ops = [
            (ArithOp::Add, "a + b"),
            (ArithOp::Sub, "a - b"),
            (ArithOp::Mul, "a * b"),
            (ArithOp::Div, "a / b"),
        ];
        for (op, want) in ops {
            check(bin(op, name("a"), name("b")), want);
        }
    }

    #[test]
    fn prints_calls_and_builders() {
        check(
            e(K::Call {
                feature: "f".into(),
                inputs: vec![name("a"), name("b")],
            }),
            "f with a, b",
        );
        check(
            e(K::BuildRecord {
                record: "Expense".into(),
                fields: vec![
                    ("name".into(), e(K::Text("L".into()))),
                    ("amount".into(), num(1_200_000)),
                ],
            }),
            "Expense with name \"L\", amount 120",
        );
        check(
            e(K::BuildList(vec![num(40_000), num(90_000)])),
            "list of 4, 9",
        );
        let call = K::Call {
            feature: "f".into(),
            inputs: vec![],
        };
        check(e(call), "f");
        let record = K::BuildRecord {
            record: "E".into(),
            fields: vec![],
        };
        check(e(record), "E");
    }

    #[test]
    fn prints_comparisons_and_logic() {
        let ops = [
            (CompareOp::Is, "a is b"),
            (CompareOp::IsNot, "a is not b"),
            (CompareOp::Greater, "a is greater than b"),
            (CompareOp::Less, "a is less than b"),
            (CompareOp::AtLeast, "a is at least b"),
            (CompareOp::AtMost, "a is at most b"),
        ];
        for (op, want) in ops {
            check(cmp(op, name("a"), name("b")), want);
        }
        for (negated, want) in [(false, "c is number"), (true, "c is not number")] {
            check(
                e(K::IsNumber {
                    value: bx(name("c")),
                    negated,
                }),
                want,
            );
        }
        check(e(K::Not(bx(name("a")))), "not a");
        check(
            e(K::And(vec![name("a"), name("b"), name("c")])),
            "a and b and c",
        );
        check(e(K::Or(vec![name("a"), name("b")])), "a or b");
    }

    #[test]
    fn keeps_parens_and_adds_none() {
        let grouped = e(K::Paren(bx(bin(ArithOp::Mul, name("a"), name("b")))));
        check(
            bin(ArithOp::Add, grouped, field_of("amount", name("x"))),
            "(a * b) + amount of x",
        );
    }
}
