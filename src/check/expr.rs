use std::collections::BTreeMap;

use super::{Ctx, Ty, combine, similar};
use crate::ast::print;
use crate::ast::{ArithOp, Conversion, Expr, ExprKind, Fact, ItemType, PlainType};
use crate::diag::Diagnostic;

pub struct Env<'a> {
    pub vars: &'a BTreeMap<String, Ty>,
    pub results: &'a BTreeMap<String, Ty>,
    pub in_test: bool,
}

pub fn plain(p: PlainType) -> Ty {
    match p {
        PlainType::Number => Ty::Number,
        PlainType::Text => Ty::Text,
        PlainType::Truth => Ty::Truth,
    }
}

pub fn item_ty(t: &ItemType) -> Ty {
    match t {
        ItemType::Plain(p) => plain(*p),
        ItemType::Record(name) => Ty::Record(name.clone()),
    }
}

pub fn report(ctx: &mut Ctx, e: &Expr, problem: String, suggest: Option<&str>) -> Ty {
    let mut d = Diagnostic::error(ctx.src, e.line, problem);
    if let Some(s) = suggest {
        d = d.suggest(s);
    }
    ctx.diags.push(d);
    Ty::Error
}

/// Types `e`, recording the type by expression id. Reports each cause once:
/// an `Error` operand makes its parent silent.
pub fn type_of(ctx: &mut Ctx, env: &Env, e: &Expr, types: &mut Vec<Option<Ty>>) -> Ty {
    let ty = infer(ctx, env, e, types);
    let slot = e.id.0 as usize;
    if types.len() <= slot {
        types.resize(slot + 1, None);
    }
    types[slot] = Some(ty.clone());
    ty
}

fn infer(ctx: &mut Ctx, env: &Env, e: &Expr, types: &mut Vec<Option<Ty>>) -> Ty {
    match &e.kind {
        ExprKind::Number(_) => Ty::Number,
        ExprKind::Text(_) => Ty::Text,
        ExprKind::Truth(_) => Ty::Truth,
        ExprKind::Name(name) => name_type(ctx, env, e, name),
        ExprKind::Shown if env.in_test => Ty::List(Box::new(Ty::Text)),
        ExprKind::Shown => report(ctx, e, "shown can only be used inside a test".into(), None),
        ExprKind::EmptyList(ItemType::Record(r))
            if !ctx.program.records().any(|d| &d.name == r) =>
        {
            combine::no_record(ctx, e, r)
        }
        ExprKind::EmptyList(item) => Ty::List(Box::new(item_ty(item))),
        ExprKind::Paren(inner) => type_of(ctx, env, inner, types),
        ExprKind::FieldOf { field, of } => {
            let t = type_of(ctx, env, of, types);
            field_type(ctx, e, field, of, t)
        }
        ExprKind::Fact { fact, of } => {
            let t = type_of(ctx, env, of, types);
            fact_type(ctx, e, *fact, of, t)
        }
        ExprKind::ItemOf { position, of } => {
            let p = type_of(ctx, env, position, types);
            let t = type_of(ctx, env, of, types);
            item_of_type(ctx, e, (position, p), (of, t))
        }
        ExprKind::Remainder { of, by } => {
            let (a, b) = (type_of(ctx, env, of, types), type_of(ctx, env, by, types));
            let (a, b) = (
                number_only(ctx, e, "remainder of", of, a),
                number_only(ctx, e, "remainder of", by, b),
            );
            if a == Ty::Error || b == Ty::Error {
                Ty::Error
            } else {
                Ty::Number
            }
        }
        ExprKind::Negate(inner) => {
            let t = type_of(ctx, env, inner, types);
            number_only(ctx, e, "negation", inner, t)
        }
        ExprKind::Convert { value, to } => {
            let t = type_of(ctx, env, value, types);
            convert_type(ctx, e, *to, value, t)
        }
        ExprKind::Binary { op, left, right } => {
            let (l, r) = (
                type_of(ctx, env, left, types),
                type_of(ctx, env, right, types),
            );
            binary_type(ctx, e, *op, (left, l), (right, r))
        }
        ExprKind::Compare { op, left, right } => {
            let (l, r) = (
                type_of(ctx, env, left, types),
                type_of(ctx, env, right, types),
            );
            combine::compare_type(ctx, e, *op, (left, l), (right, r))
        }
        ExprKind::IsNumber { value, negated } => {
            let t = type_of(ctx, env, value, types);
            combine::is_number_type(ctx, e, value, *negated, t)
        }
        ExprKind::Not(inner) => {
            let t = type_of(ctx, env, inner, types);
            combine::condition_type(ctx, e, "not works on a condition", &[(inner, t)])
        }
        ExprKind::And(parts) | ExprKind::Or(parts) => {
            let word = if matches!(e.kind, ExprKind::And(_)) {
                "and joins conditions"
            } else {
                "or joins conditions"
            };
            let tys = combine::type_all(ctx, env, parts, types);
            let pairs: Vec<(&Expr, Ty)> = parts.iter().zip(tys).collect();
            combine::condition_type(ctx, e, word, &pairs)
        }
        ExprKind::Call { feature, inputs } => {
            let tys = combine::type_all(ctx, env, inputs, types);
            let args: Vec<(&Expr, Ty)> = inputs.iter().zip(tys).collect();
            combine::call_type(ctx, env, e, feature, &args)
        }
        ExprKind::BuildRecord { record, fields } => {
            let given: Vec<(&str, &Expr, Ty)> = fields
                .iter()
                .map(|(n, x)| (n.as_str(), x, type_of(ctx, env, x, types)))
                .collect();
            combine::record_type(ctx, e, record, &given)
        }
        ExprKind::BuildList(items) => {
            let tys = combine::type_all(ctx, env, items, types);
            let pairs: Vec<(&Expr, Ty)> = items.iter().zip(tys).collect();
            combine::list_type(ctx, e, &pairs)
        }
    }
}

fn binary_type(ctx: &mut Ctx, e: &Expr, op: ArithOp, l: (&Expr, Ty), r: (&Expr, Ty)) -> Ty {
    let sym = match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    };
    let bad = [l.clone(), r.clone()]
        .into_iter()
        .find(|(_, t)| !matches!(t, Ty::Number | Ty::Error));
    if let Some((operand, t)) = bad {
        let msg = format!(
            "{sym} needs numbers on both sides; {} is {}",
            print::expr(operand),
            t.described()
        );
        return report(ctx, e, msg, None);
    }
    if l.1 == Ty::Error || r.1 == Ty::Error {
        Ty::Error
    } else {
        Ty::Number
    }
}

fn convert_type(ctx: &mut Ctx, e: &Expr, to: Conversion, value: &Expr, t: Ty) -> Ty {
    let shown = print::expr(value);
    let (result, problem) = match (to, &t) {
        (_, Ty::Error) => return Ty::Error,
        (Conversion::ToNumber, Ty::Text) => (Ty::Number, None),
        (Conversion::ToNumber, Ty::Number) => (
            Ty::Number,
            Some(format!(
                "as number converts text; {shown} is already a number"
            )),
        ),
        (Conversion::ToNumber, _) => (
            Ty::Number,
            Some(format!(
                "as number converts text; {shown} is {}",
                t.described()
            )),
        ),
        (Conversion::ToText, Ty::Number | Ty::Truth) => (Ty::Text, None),
        (Conversion::ToText, Ty::Text) => (Ty::Text, Some(format!("{shown} is already text"))),
        (Conversion::ToText, _) => (
            Ty::Text,
            Some(format!(
                "Only numbers and truths can be converted to text; {shown} is {}",
                t.described()
            )),
        ),
        (Conversion::ToTextPlaces(_), Ty::Number) => (Ty::Text, None),
        (Conversion::ToTextPlaces(_), _) => (
            Ty::Text,
            Some(format!(
                "with places works on numbers; {shown} is {}",
                t.described()
            )),
        ),
    };
    match problem {
        None => result,
        Some(msg) => report(ctx, e, msg, None),
    }
}

/// Requires a number; reports once if the operand is a settled non-number.
fn number_only(ctx: &mut Ctx, e: &Expr, what: &str, operand: &Expr, t: Ty) -> Ty {
    match t {
        Ty::Number | Ty::Error => t,
        other => {
            let msg = format!(
                "{what} works on numbers; {} is {}",
                print::expr(operand),
                other.described()
            );
            report(ctx, e, msg, None)
        }
    }
}

fn name_type(ctx: &mut Ctx, env: &Env, e: &Expr, name: &str) -> Ty {
    if let Some(t) = env.vars.get(name) {
        return t.clone();
    }
    if let Some(t) = ctx.program.tables().find(|t| t.name == name) {
        return Ty::Table {
            table: t.name.clone(),
            record: t.record.clone(),
        };
    }
    if let Some(f) = ctx.program.features().find(|f| f.name == name) {
        if f.needs.is_empty() {
            return env.results.get(&f.name).cloned().unwrap_or(Ty::Error);
        }
        let inputs: Vec<&str> = f.needs.iter().map(|n| n.name.as_str()).collect();
        let msg = format!(
            "{}, as in {name} with ...",
            combine::needs_phrase(name, &inputs)
        );
        return report(ctx, e, msg, None);
    }
    let candidates = env
        .vars
        .keys()
        .map(String::as_str)
        .chain(ctx.program.tables().map(|t| t.name.as_str()))
        .chain(ctx.program.features().map(|f| f.name.as_str()));
    let near = similar::nearest(name, candidates).map(str::to_string);
    report(ctx, e, format!("{name} has no value"), near.as_deref())
}

fn field_type(ctx: &mut Ctx, e: &Expr, field: &str, of: &Expr, t: Ty) -> Ty {
    match &t {
        Ty::Error => Ty::Error,
        Ty::Record(rec) => {
            let Some(decl) = ctx.program.records().find(|r| &r.name == rec) else {
                return Ty::Error;
            };
            if let Some(f) = decl.fields().find(|f| f.name == field) {
                return plain(f.ty);
            }
            let near =
                similar::nearest(field, decl.fields().map(|f| f.name.as_str())).map(str::to_string);
            report(
                ctx,
                e,
                format!("{rec} has no field {field}"),
                near.as_deref(),
            )
        }
        other => {
            let msg = format!(
                "{} is {}, so it has no field {field}",
                print::expr(of),
                other.described()
            );
            report(ctx, e, msg, None)
        }
    }
}

fn fact_type(ctx: &mut Ctx, e: &Expr, fact: Fact, of: &Expr, t: Ty) -> Ty {
    if t == Ty::Error {
        return Ty::Error;
    }
    let (word, listy) = match fact {
        Fact::Count => ("count", true),
        Fact::First => ("first", true),
        Fact::Last => ("last", true),
        Fact::Whole => ("whole", false),
    };
    if !listy {
        return match number_only(ctx, e, "whole of", of, t) {
            Ty::Error => Ty::Error,
            _ => Ty::Number,
        };
    }
    match (fact, t.item()) {
        (Fact::Count, Some(_)) => Ty::Number,
        (_, Some(item)) => item,
        (_, None) => {
            let msg = format!(
                "{word} of works on lists and tables; {} is {}",
                print::expr(of),
                t.described()
            );
            report(ctx, e, msg, None)
        }
    }
}

fn item_of_type(ctx: &mut Ctx, e: &Expr, pos: (&Expr, Ty), of: (&Expr, Ty)) -> Ty {
    let (pos_e, pos_t) = pos;
    let (of_e, of_t) = of;
    let mut bad = pos_t == Ty::Error || of_t == Ty::Error;
    if !matches!(pos_t, Ty::Number | Ty::Error) {
        let msg = format!(
            "The position in item must be a number; {} is {}",
            print::expr(pos_e),
            pos_t.described()
        );
        report(ctx, e, msg, None);
        bad = true;
    }
    match of_t.item() {
        Some(item) if !bad => item,
        Some(_) => Ty::Error,
        None => {
            if of_t != Ty::Error {
                let msg = format!(
                    "item works on lists and tables; {} is {}",
                    print::expr(of_e),
                    of_t.described()
                );
                report(ctx, e, msg, None);
            }
            Ty::Error
        }
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }
    fn body(lines: &str) -> Vec<String> {
        diags(&format!(
            "record E\n  amount as number\n  name as text\ntable es of E in \"e.csv\"\nfeature start\n{lines}"
        ))
    }

    #[test]
    fn names_have_values() {
        assert!(body("  set n to 1\n  show n, es, \"a\", yes\n").is_empty());
        assert!(diags("feature start\n  show 1\ntest \"t\"\n  show shown\n").is_empty());
        assert_eq!(
            body("  show totl\n"),
            ["line 6: show totl. totl has no value."]
        );
        assert_eq!(
            body("  set total to 1\n  show total, totl\n"),
            ["line 7: show total, totl. totl has no value. Did you mean total?"]
        );
    }

    #[test]
    fn values_and_facts() {
        assert!(
            body(
                "  set n to 1\n  show n + 2, count of es, amount of first of es, whole of n, -n\n"
            )
            .is_empty()
        );
        assert_eq!(
            body("  show totl\n"),
            ["line 6: show totl. totl has no value."]
        );
        assert_eq!(
            body("  set total to 1\n  show total, totl\n"),
            ["line 7: show total, totl. totl has no value. Did you mean total?"]
        );
        assert_eq!(
            body("  set e to first of es\n  show amout of e\n"),
            ["line 7: show amout of e. E has no field amout. Did you mean amount?"]
        );
        assert_eq!(
            body("  set n to 1\n  show amount of n\n"),
            ["line 7: show amount of n. n is a number, so it has no field amount."]
        );
        assert_eq!(
            body("  set n to 1\n  show count of n\n"),
            ["line 7: show count of n. count of works on lists and tables; n is a number."]
        );
        assert_eq!(
            body("  show whole of \"a\"\n"),
            ["line 6: show whole of \"a\". whole of works on numbers; \"a\" is text."]
        );
        assert_eq!(
            body("  show item (\"a\") of es\n"),
            [
                "line 6: show item (\"a\") of es. The position in item must be a number; (\"a\") is text."
            ]
        );
    }

    #[test]
    fn arithmetic_and_conversion() {
        assert_eq!(
            body("  show 1 + \"a\"\n"),
            ["line 6: show 1 + \"a\". + needs numbers on both sides; \"a\" is text."]
        );
        assert_eq!(
            body("  show 1 as number\n"),
            ["line 6: show 1 as number. as number converts text; 1 is already a number."]
        );
        assert_eq!(
            body("  show \"a\" as text\n"),
            ["line 6: show \"a\" as text. \"a\" is already text."]
        );
        assert_eq!(
            body("  show \"a\" as text with 2 places\n"),
            [
                "line 6: show \"a\" as text with 2 places. with places works on numbers; \"a\" is text."
            ]
        );
        assert_eq!(
            body("  show first of es as text\n"),
            [
                "line 6: show first of es as text. Only numbers and truths can be converted to text; first of es is E."
            ]
        );
        assert!(body("  show yes as text, 1 as text with 2 places, \"3\" as number\n").is_empty());
        assert_eq!(
            body("  show 1 + (2 + \"a\")\n"),
            ["line 6: show 1 + (2 + \"a\"). + needs numbers on both sides; \"a\" is text."]
        );
    }
}
