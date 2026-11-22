use super::expr::{Env, item_ty, report, type_of};
use super::{Ctx, Ty, similar};
use crate::ast::print;
use crate::ast::{CompareOp, Expr, Type};

pub fn compare_type(ctx: &mut Ctx, e: &Expr, op: CompareOp, l: (&Expr, Ty), r: (&Expr, Ty)) -> Ty {
    if l.1 == Ty::Error || r.1 == Ty::Error {
        return Ty::Error;
    }
    let word = match op {
        CompareOp::Is => "is",
        CompareOp::IsNot => "is not",
        CompareOp::Greater => "is greater than",
        CompareOp::Less => "is less than",
        CompareOp::AtLeast => "is at least",
        CompareOp::AtMost => "is at most",
    };
    let problem = if matches!(op, CompareOp::Is | CompareOp::IsNot) {
        (l.1 != r.1).then(|| {
            format!(
                "{word} compares two values of the same type; {} is {} and {} is {}",
                print::expr(l.0),
                l.1.described(),
                print::expr(r.0),
                r.1.described()
            )
        })
    } else {
        [&l, &r]
            .into_iter()
            .find(|(_, t)| *t != Ty::Number)
            .map(|(x, t)| {
                format!(
                    "{word} compares numbers; {} is {}",
                    print::expr(x),
                    t.described()
                )
            })
    };
    match problem {
        Some(msg) => report(ctx, e, msg, None),
        None => Ty::Truth,
    }
}

pub fn is_number_type(ctx: &mut Ctx, e: &Expr, value: &Expr, negated: bool, t: Ty) -> Ty {
    let word = if negated {
        "is not number"
    } else {
        "is number"
    };
    match t {
        Ty::Text => Ty::Truth,
        Ty::Error => Ty::Error,
        other => {
            let what = if other == Ty::Number {
                format!("{} is already a number", print::expr(value))
            } else {
                format!("{} is {}", print::expr(value), other.described())
            };
            report(ctx, e, format!("{word} checks text; {what}"), None)
        }
    }
}

/// Requires every operand to be a truth; reports the first one that is not.
pub fn condition_type(ctx: &mut Ctx, e: &Expr, word: &str, parts: &[(&Expr, Ty)]) -> Ty {
    if parts.iter().any(|(_, t)| *t == Ty::Error) {
        return Ty::Error;
    }
    match parts.iter().find(|(_, t)| *t != Ty::Truth) {
        Some((x, t)) => {
            let msg = format!("{word}; {} is {}", print::expr(x), t.described());
            report(ctx, e, msg, None)
        }
        None => Ty::Truth,
    }
}

pub fn type_all(ctx: &mut Ctx, env: &Env, es: &[Expr], types: &mut Vec<Option<Ty>>) -> Vec<Ty> {
    es.iter().map(|x| type_of(ctx, env, x, types)).collect()
}

pub fn names(list: &[&str]) -> String {
    match list.split_last() {
        Some((last, head)) if !head.is_empty() => format!("{} and {last}", head.join(", ")),
        _ => list.join(""),
    }
}

/// "f needs 1 input: n", "f needs 2 inputs: n and t" or "f needs no inputs".
pub fn needs_phrase(feature: &str, inputs: &[&str]) -> String {
    match inputs.len() {
        0 => format!("{feature} needs no inputs"),
        1 => format!("{feature} needs 1 input: {}", inputs[0]),
        n => format!("{feature} needs {n} inputs: {}", names(inputs)),
    }
}

/// A table is accepted where a list of its record is needed.
fn accepts(want: &Ty, got: &Ty) -> bool {
    match (want, got) {
        (Ty::List(item), Ty::Table { record, .. }) => **item == Ty::Record(record.clone()),
        _ => want == got,
    }
}

pub fn call_type(ctx: &mut Ctx, env: &Env, e: &Expr, feature: &str, args: &[(&Expr, Ty)]) -> Ty {
    let program = ctx.program;
    let Some(decl) = program.features().find(|f| f.name == feature) else {
        let near = similar::nearest(feature, program.features().map(|f| f.name.as_str()));
        let msg = format!("There is no feature named {feature}");
        return report(ctx, e, msg, near);
    };
    let input_names: Vec<&str> = decl.needs.iter().map(|n| n.name.as_str()).collect();
    if decl.needs.len() != args.len() {
        let msg = format!(
            "{}; this call gives {}",
            needs_phrase(feature, &input_names),
            args.len()
        );
        return report(ctx, e, msg, None);
    }
    for (input, (arg, got)) in decl.needs.iter().zip(args) {
        let want = match &input.ty {
            Type::Item(t) => item_ty(t),
            Type::List(t) => Ty::List(Box::new(item_ty(t))),
        };
        if *got != Ty::Error && !accepts(&want, got) {
            let msg = format!(
                "The input {} of {feature} is {}; {} is {}",
                input.name,
                want.described(),
                print::expr(arg),
                got.described()
            );
            report(ctx, e, msg, None);
        }
    }
    env.results.get(feature).cloned().unwrap_or(Ty::Error)
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }
    fn with_f(lines: &str) -> Vec<String> {
        diags(&format!(
            "record E\n  amount as number\n  name as text\ntable es of E in \"e.csv\"\nfeature f\n  needs n as number, t as text\n  give n\nfeature g\n  needs xs as list of E\n  give 1\nfeature start\n{lines}"
        ))
    }

    #[test]
    fn comparisons_and_logic() {
        assert!(
            with_f("  show ((1 is 2) and not (\"a\" is not \"b\")), (\"3\" is number)\n")
                .is_empty()
        );
        assert_eq!(
            with_f("  show (1 is \"a\")\n"),
            [
                "line 12: show (1 is \"a\"). is compares two values of the same type; 1 is a number and \"a\" is text."
            ]
        );
        assert_eq!(
            with_f("  show (\"a\" is greater than \"b\")\n"),
            [
                "line 12: show (\"a\" is greater than \"b\"). is greater than compares numbers; \"a\" is text."
            ]
        );
        assert_eq!(
            with_f("  show (1 is number)\n"),
            ["line 12: show (1 is number). is number checks text; 1 is already a number."]
        );
        assert_eq!(
            with_f("  show (1 and yes)\n"),
            ["line 12: show (1 and yes). and joins conditions; 1 is a number."]
        );
        assert_eq!(
            with_f("  show (not 1)\n"),
            ["line 12: show (not 1). not works on a condition; 1 is a number."]
        );
    }

    #[test]
    fn or_and_record_equality() {
        assert_eq!(
            with_f("  show (1 or yes)\n"),
            ["line 12: show (1 or yes). or joins conditions; 1 is a number."]
        );
        let two = "record E\n  a as number\nrecord S\n  b as number\ntable es of E in \"e.csv\"\ntable ss of S in \"s.csv\"\nfeature start\n";
        let src = |l: &str| diags(&format!("{two}{l}"));
        assert!(src("  show (first of es is first of es)\n").is_empty());
        assert_eq!(
            src("  show (first of es is first of ss)\n"),
            [
                "line 8: show (first of es is first of ss). is compares two values of the same type; first of es is E and first of ss is S."
            ]
        );
    }

    #[test]
    fn calls_and_building() {
        assert!(with_f("  show (f with 1, \"a\"), (g with es), (g with (list of (E with amount 1, name \"x\")))\n").is_empty());
        assert_eq!(
            with_f("  show (f with 1)\n"),
            ["line 12: show (f with 1). f needs 2 inputs: n and t; this call gives 1."]
        );
        assert_eq!(
            with_f("  show (f with \"a\", \"b\")\n"),
            ["line 12: show (f with \"a\", \"b\"). The input n of f is a number; \"a\" is text."]
        );
        assert_eq!(
            with_f("  show (h with 1)\n"),
            ["line 12: show (h with 1). There is no feature named h."]
        );
        assert_eq!(
            with_f("  show f\n"),
            ["line 12: show f. f needs 2 inputs: n and t, as in f with ...."]
        );
    }

    #[test]
    fn call_arity_and_table_inputs() {
        let src = |l: &str| {
            diags(&format!(
                "record E\n  amount as number\nrecord S\n  b as number\ntable es of E in \"e.csv\"\nfeature one\n  needs n as number\n  give n\nfeature zero\n  give 1\nfeature takes_s\n  needs xs as list of S\n  give 1\nfeature takes_n\n  needs xs as list of number\n  give 1\nfeature start\n{l}"
            ))
        };
        assert_eq!(
            src("  show one\n"),
            ["line 18: show one. one needs 1 input: n, as in one with ...."]
        );
        assert_eq!(
            src("  show (zero with 1)\n"),
            ["line 18: show (zero with 1). zero needs no inputs; this call gives 1."]
        );
        assert_eq!(
            src("  show (takes_s with es)\n"),
            [
                "line 18: show (takes_s with es). The input xs of takes_s is list of S; es is table es."
            ]
        );
        assert_eq!(
            src("  show (takes_n with es)\n"),
            [
                "line 18: show (takes_n with es). The input xs of takes_n is list of number; es is table es."
            ]
        );
    }
}
