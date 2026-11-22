pub use super::stmt_sexpr::stmt;
use crate::ast::{ArithOp, CompareOp, Conversion, Expr, ExprKind as K, Fact, item_type_text};

pub fn sexpr(e: &Expr) -> String {
    match &e.kind {
        K::Number(n) => n.show(),
        K::Text(t) => format!("\"{}\"", t.replace('"', "\"\"")),
        K::Truth(b) => if *b { "yes" } else { "no" }.into(),
        K::Name(n) => n.clone(),
        K::Shown => "shown".into(),
        K::EmptyList(t) => format!("(empty-list {})", item_type_text(t)),
        K::Paren(x) => form("paren", &[x]),
        K::FieldOf { field, of } => form(&format!("of {field}"), &[of]),
        K::Fact { fact, of } => form(fact_name(*fact), &[of]),
        K::ItemOf { position, of } => form("item", &[position, of]),
        K::Remainder { of, by } => form("remainder", &[of, by]),
        K::Negate(x) => form("-", &[x]),
        K::Convert { value, to } => form(&conversion_name(*to), &[value]),
        K::Binary { op, left, right } => form(arith_op(*op), &[left, right]),
        K::Call { feature, inputs } => list(&format!("call {feature}"), inputs),
        K::BuildRecord { record, fields } => {
            let fs: String = fields
                .iter()
                .map(|(n, v)| format!(" ({n} {})", sexpr(v)))
                .collect();
            format!("(record {record}{fs})")
        }
        K::BuildList(xs) => list("list", xs),
        K::Compare { op, left, right } => form(compare_op(*op), &[left, right]),
        K::IsNumber { value, negated } => form(
            if *negated {
                "is-not-number"
            } else {
                "is-number"
            },
            &[value],
        ),
        K::Not(x) => form("not", &[x]),
        K::And(xs) => list("and", xs),
        K::Or(xs) => list("or", xs),
    }
}

fn fact_name(f: Fact) -> &'static str {
    match f {
        Fact::Count => "count",
        Fact::First => "first",
        Fact::Last => "last",
        Fact::Whole => "whole",
    }
}

fn arith_op(op: ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    }
}

fn compare_op(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Is => "is",
        CompareOp::IsNot => "is-not",
        CompareOp::Greater => "greater",
        CompareOp::Less => "less",
        CompareOp::AtLeast => "at-least",
        CompareOp::AtMost => "at-most",
    }
}

fn conversion_name(c: Conversion) -> String {
    match c {
        Conversion::ToNumber => "as-number".into(),
        Conversion::ToText => "as-text".into(),
        Conversion::ToTextPlaces(n) => format!("as-text-{n}"),
    }
}

fn form(head: &str, args: &[&Expr]) -> String {
    let parts: String = args.iter().map(|a| format!(" {}", sexpr(a))).collect();
    format!("({head}{parts})")
}

fn list(head: &str, xs: &[Expr]) -> String {
    form(head, &xs.iter().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ExprId, ItemType};
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
        assert_eq!(sexpr(&x), want, "{x:?}");
    }

    #[test]
    fn prints_leaves_and_wrappers() {
        let student = ItemType::Record("Student".into());
        check(num(1_205_000), "120.5");
        check(e(K::Text("a\"b".into())), "\"a\"\"b\"");
        check(e(K::Truth(true)), "yes");
        check(e(K::Truth(false)), "no");
        check(name("x"), "x");
        check(e(K::Shown), "shown");
        check(e(K::EmptyList(student)), "(empty-list Student)");
        check(e(K::Paren(bx(name("a")))), "(paren a)");
        check(e(K::Negate(bx(name("x")))), "(- x)");
        check(e(K::Not(bx(name("a")))), "(not a)");
        check(e(K::BuildList(vec![name("a")])), "(list a)");
    }

    #[test]
    fn prints_compound_forms() {
        let pair = || vec![name("a"), name("b")];
        let (n, xs) = (bx(name("n")), bx(name("xs")));
        check(
            e(K::FieldOf {
                field: "amount".into(),
                of: bx(name("x")),
            }),
            "(of amount x)",
        );
        check(
            e(K::ItemOf {
                position: bx(num(20_000)),
                of: xs,
            }),
            "(item 2 xs)",
        );
        check(
            e(K::Remainder {
                of: n,
                by: bx(num(20_000)),
            }),
            "(remainder n 2)",
        );
        check(
            e(K::Call {
                feature: "f".into(),
                inputs: pair(),
            }),
            "(call f a b)",
        );
        let fields = vec![("name".into(), name("n"))];
        check(
            e(K::BuildRecord {
                record: "E".into(),
                fields,
            }),
            "(record E (name n))",
        );
        check(e(K::And(pair())), "(and a b)");
        check(e(K::Or(pair())), "(or a b)");
        for (negated, want) in [(true, "(is-not-number c)"), (false, "(is-number c)")] {
            check(
                e(K::IsNumber {
                    value: bx(name("c")),
                    negated,
                }),
                want,
            );
        }
    }

    #[test]
    fn prints_every_fact_and_conversion() {
        let facts = [
            (Fact::Count, "count"),
            (Fact::First, "first"),
            (Fact::Last, "last"),
            (Fact::Whole, "whole"),
        ];
        for (fact, w) in facts {
            check(
                e(K::Fact {
                    fact,
                    of: bx(name("x")),
                }),
                &format!("({w} x)"),
            );
        }
        let mut convs = vec![
            (Conversion::ToNumber, "as-number".to_string()),
            (Conversion::ToText, "as-text".to_string()),
        ];
        for n in 0..=4 {
            convs.push((Conversion::ToTextPlaces(n), format!("as-text-{n}")));
        }
        for (to, w) in convs {
            check(
                e(K::Convert {
                    value: bx(name("t")),
                    to,
                }),
                &format!("({w} t)"),
            );
        }
    }

    #[test]
    fn prints_every_arithmetic_and_comparison_op() {
        let ariths = [
            (ArithOp::Add, "+"),
            (ArithOp::Sub, "-"),
            (ArithOp::Mul, "*"),
            (ArithOp::Div, "/"),
        ];
        for (op, w) in ariths {
            check(bin(op, name("a"), name("b")), &format!("({w} a b)"));
        }
        let cmps = [
            (CompareOp::Is, "is"),
            (CompareOp::IsNot, "is-not"),
            (CompareOp::Greater, "greater"),
            (CompareOp::Less, "less"),
            (CompareOp::AtLeast, "at-least"),
            (CompareOp::AtMost, "at-most"),
        ];
        for (op, w) in cmps {
            check(cmp(op, name("a"), name("b")), &format!("({w} a b)"));
        }
    }
}
