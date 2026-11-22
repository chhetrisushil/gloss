use std::collections::HashMap;

use super::{Ctx, similar};
use crate::ast::{ItemType, Type};
use crate::diag::Diagnostic;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Record,
    Table,
    Feature,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::Record => "a record",
            Kind::Table => "a table",
            Kind::Feature => "a feature",
        }
    }
}

pub fn check_program(ctx: &mut Ctx) {
    start_rules(ctx);
    program_names(ctx);
    fields(ctx);
    test_titles(ctx);
    unknown_records(ctx);
}

fn start_rules(ctx: &mut Ctx) {
    match ctx.program.features().find(|f| f.name == "start") {
        None => ctx.diags.push(Diagnostic::program(
            "This program has no feature start, so it has nowhere to begin",
        )),
        Some(f) => {
            if let Some(n) = f.needs.first() {
                let msg = "start cannot have a needs line; it is where the program begins";
                ctx.diags.push(Diagnostic::error(ctx.src, n.line, msg));
            }
        }
    }
}

fn program_names(ctx: &mut Ctx) {
    let mut names: Vec<(&str, Kind, u32)> = Vec::new();
    names.extend(
        ctx.program
            .records()
            .map(|r| (&*r.name, Kind::Record, r.line)),
    );
    names.extend(
        ctx.program
            .tables()
            .map(|t| (&*t.name, Kind::Table, t.line)),
    );
    names.extend(
        ctx.program
            .features()
            .map(|f| (&*f.name, Kind::Feature, f.line)),
    );
    names.sort_by_key(|n| n.2);
    for (i, &(name, kind, line)) in names.iter().enumerate() {
        let earlier = &names[..i];
        let problem = if let Some(&(_, k, l)) = earlier.iter().find(|n| n.0 == name) {
            format!("{name} is already used on line {l} for {}", k.noun())
        } else if let Some(&(other, ..)) = earlier
            .iter()
            .find(|&&(o, k, _)| collides(name, kind, o, k))
        {
            format!("{name} and {other} differ only in case; rename one")
        } else {
            continue;
        };
        ctx.diags.push(Diagnostic::error(ctx.src, line, problem));
    }
}

/// Equal when lowercased, except a record and a name that differs only in the first letter.
fn collides(a: &str, ka: Kind, b: &str, kb: Kind) -> bool {
    if a.to_lowercase() != b.to_lowercase() {
        return false;
    }
    let tail = |s: &str| s.chars().skip(1).collect::<String>();
    let exempt = (ka == Kind::Record) != (kb == Kind::Record) && tail(a) == tail(b);
    !exempt
}

fn fields(ctx: &mut Ctx) {
    for r in ctx.program.records() {
        let mut seen: HashMap<&str, u32> = HashMap::new();
        for f in r.fields() {
            if seen.insert(&f.name, f.line).is_some() {
                let msg = format!("{} already has a field named {}", r.name, f.name);
                ctx.diags.push(Diagnostic::error(ctx.src, f.line, msg));
            }
        }
    }
}

fn test_titles(ctx: &mut Ctx) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for t in ctx.program.tests() {
        if let Some(first) = seen.get(t.title.as_str()) {
            let msg = format!("Another test already has this title (line {first})");
            ctx.diags.push(Diagnostic::error(ctx.src, t.line, msg));
        } else {
            seen.insert(&t.title, t.line);
        }
    }
}

fn unknown_records(ctx: &mut Ctx) {
    let used = ctx.program.tables().map(|t| (t.line, &t.record)).chain(
        ctx.program
            .features()
            .flat_map(|f| &f.needs)
            .filter_map(|n| match &n.ty {
                Type::Item(ItemType::Record(r)) | Type::List(ItemType::Record(r)) => {
                    Some((n.line, r))
                }
                _ => None,
            }),
    );
    let known: Vec<&str> = ctx.program.records().map(|r| r.name.as_str()).collect();
    for (line, name) in used {
        if known.iter().any(|k| k == name) {
            continue;
        }
        let mut d = Diagnostic::error(ctx.src, line, format!("There is no record named {name}"));
        if let Some(s) = similar::nearest(name, known.iter().copied()) {
            d = d.suggest(s);
        }
        ctx.diags.push(d);
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }
    const START: &str = "feature start\n  show 1\n";

    #[test]
    fn start_rules() {
        assert_eq!(
            diags("feature main\n  show 1\n"),
            ["This program has no feature start, so it has nowhere to begin."]
        );
        assert_eq!(
            diags("feature start\n  needs a as number\n  show 1\n"),
            [
                "line 2: needs a as number. start cannot have a needs line; it is where the program begins."
            ]
        );
    }

    #[test]
    fn duplicates_and_case() {
        let p = format!("{START}feature report\n  show 1\nfeature report\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: feature report. report is already used on line 3 for a feature."]
        );
        let p = format!("{START}feature myTotal\n  show 1\nfeature mytotal\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: feature mytotal. mytotal and myTotal differ only in case; rename one."]
        );
        let p = format!("{START}record E\n  a as text\n  a as number\n");
        assert_eq!(
            diags(&p),
            ["line 5: a as number. E already has a field named a."]
        );
        let p = format!("{START}test \"x\"\n  show 1\ntest \"x\"\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: test \"x\". Another test already has this title (line 3)."]
        );
    }

    #[test]
    fn unknown_records_suggest() {
        let p = format!("{START}record Expense\n  a as text\ntable t of Expens in \"x.csv\"\n");
        assert_eq!(
            diags(&p),
            [
                r#"line 5: table t of Expens in "x.csv". There is no record named Expens. Did you mean Expense?"#
            ]
        );
    }

    #[test]
    fn collides_exemption_and_cross_kind() {
        let p = format!("{START}record Expense\n  a as text\nfeature expense\n  show 1\n");
        assert!(diags(&p).is_empty());
        let p = format!("{START}record Expense\n  a as text\nrecord EXPENSE\n  a as text\n");
        assert_eq!(
            diags(&p),
            ["line 5: record EXPENSE. EXPENSE and Expense differ only in case; rename one."]
        );
        let p = format!(
            "{START}record E\n  a as text\ntable report of E in \"x.csv\"\nfeature report\n  show 1\n"
        );
        assert_eq!(
            diags(&p),
            ["line 6: feature report. report is already used on line 5 for a table."]
        );
    }

    #[test]
    fn unknown_list_input_suggests() {
        let p = format!(
            "{START}record Expense\n  a as text\nfeature f\n  needs x as list of Expens\n  show 1\n"
        );
        assert_eq!(
            diags(&p),
            [
                "line 6: needs x as list of Expens. There is no record named Expens. Did you mean Expense?"
            ]
        );
    }
}
