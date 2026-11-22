use std::collections::BTreeMap;

use super::expr::{item_ty, report};
use super::walking::{blocks, called};
use super::{Ctx, FeatureInfo, Ty, stmt};
use crate::ast::program::FeatureDecl;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::{Expr, Line, Type, print};
use crate::diag::Diagnostic;

/// The declared inputs of a feature with their types.
pub fn inputs_of(f: &FeatureDecl) -> Vec<(String, Ty)> {
    let typed = |t: &Type| match t {
        Type::Item(t) => item_ty(t),
        Type::List(t) => Ty::List(Box::new(item_ty(t))),
    };
    f.needs
        .iter()
        .map(|n| (n.name.clone(), typed(&n.ty)))
        .collect()
}

/// What every feature gives. A result starts as `Error`, meaning "being worked
/// out", and bodies are typed silently until nothing changes, so a feature can
/// be used before it is declared, or call itself.
pub fn infer_results(ctx: &mut Ctx) -> BTreeMap<String, Ty> {
    let program = ctx.program;
    let mut results: BTreeMap<String, Ty> = program
        .features()
        .map(|f| (f.name.clone(), Ty::Error))
        .collect();
    // Each pass that changes anything moves some feature's result to the type of an earlier
    // give (or settles it for the first time), and a feature can do that at most once per
    // give it has, so the passes cannot outnumber the gives. One more pass sees no change,
    // and one more covers features with no give, which settle on `Nothing` in the first pass.
    let passes = program.features().map(|f| gives_in(&f.body)).sum::<usize>() + 2;
    let mut converged = false;
    for _ in 0..passes {
        let mut changed = false;
        for f in program.features() {
            let kept = ctx.diags.len();
            let inputs = inputs_of(f);
            let checked = stmt::check_body(ctx, &results, &inputs, &f.body, false, &mut Vec::new());
            ctx.diags.truncate(kept);
            changed |=
                results.insert(f.name.clone(), checked.result.clone()) != Some(checked.result);
        }
        if !changed {
            converged = true;
            break;
        }
    }
    debug_assert!(converged, "feature results did not settle");
    results
}

/// How many give statements a body holds, nested ones included.
fn gives_in(body: &[Stmt]) -> usize {
    let nested = |s: &Stmt| blocks(&s.kind).into_iter().map(gives_in).sum::<usize>();
    body.iter()
        .map(|s| usize::from(matches!(s.kind, StmtKind::Give(_))) + nested(s))
        .sum()
}

/// Reports a feature whose result is still unknown with nothing else wrong.
pub fn check_result(ctx: &mut Ctx, f: &FeatureDecl, checked: &FeatureInfo, kept: usize) {
    // A cycle is reported even beside unrelated errors, unless a give already has its own.
    // A feature merely waiting on a broken one is not in a cycle; the broken one reports itself.
    let unknown = checked.cycle || (checked.result == Ty::Error && ctx.diags.len() == kept);
    if unknown && reaches_itself(ctx, &f.name) {
        let name = &f.name;
        let msg =
            format!("cannot work out what {name} gives; add a give that does not call {name}");
        ctx.diags.push(Diagnostic::error(ctx.src, f.line, msg));
    }
}

/// Every feature called anywhere in a body, nested blocks included.
fn calls_in<'a>(body: &'a [Stmt], out: &mut Vec<&'a str>) {
    for stmt in body {
        called(&stmt.kind, out);
        blocks(&stmt.kind)
            .into_iter()
            .for_each(|b| calls_in(b, out));
    }
}

/// True when `name` can call itself, directly or through other features. Any call counts,
/// not only those in gives, because a give can use a value set from a call.
fn reaches_itself(ctx: &Ctx, name: &str) -> bool {
    let program = ctx.program;
    let mut todo = vec![name];
    let mut seen = Vec::new();
    while let Some(current) = todo.pop() {
        let Some(feature) = program.features().find(|f| f.name == current) else {
            continue;
        };
        let mut calls = Vec::new();
        calls_in(&feature.body, &mut calls);
        for next in calls {
            if next == name {
                return true;
            }
            if !seen.contains(&next) {
                seen.push(next);
                todo.push(next);
            }
        }
    }
    false
}

/// True when the last statement gives, or an if with an otherwise whose every branch does.
fn always_gives(body: &[Stmt]) -> bool {
    let last = body
        .iter()
        .rev()
        .find(|s| !matches!(s.kind, StmtKind::Note(_)));
    match last.map(|s| &s.kind) {
        Some(StmtKind::Give(_)) => true,
        Some(StmtKind::If {
            branches,
            otherwise: Some(other),
        }) => branches.iter().all(|(_, b)| always_gives(b)) && always_gives(other),
        _ => false,
    }
}

/// Checks the gives of one feature: `start` has none, they agree, and every path reaches one.
pub fn check_gives(ctx: &mut Ctx, f: &FeatureDecl, checked: &FeatureInfo) {
    let mut error = |line, msg: String| ctx.diags.push(Diagnostic::error(ctx.src, line, msg));
    if f.name == "start" {
        for (line, _) in &checked.gives {
            error(
                *line,
                "start cannot give a value; it is where the program begins".into(),
            );
        }
        return;
    }
    let mut known = checked.gives.iter().filter(|(_, t)| *t != Ty::Error);
    if let Some((first, want)) = known.next() {
        for (line, got) in known.filter(|(_, t)| t != want) {
            let msg = format!(
                "Every give in {} must give the same type: line {first} gives {}, this line gives {}",
                f.name,
                want.described(),
                got.described()
            );
            error(*line, msg);
        }
    }
    if !checked.gives.is_empty() && !always_gives(&f.body) {
        error(
            f.line,
            format!("{} can reach its end without giving a value", f.name),
        );
    }
}

/// A feature used as a value must give one.
pub fn as_value(ctx: &mut Ctx, e: &Expr, feature: &str, ty: Ty) -> Ty {
    if ty != Ty::Nothing {
        return ty;
    }
    let msg =
        format!("{feature} gives nothing, so it cannot be used as a value; use run {feature}");
    report(ctx, e, msg, None)
}

/// `run` of a feature that gives a value: say how to use the value, from the user's own call.
pub fn check_run(ctx: &mut Ctx, line: Line, feature: &str, inputs: &[Expr], ty: &Ty) {
    if matches!(ty, Ty::Nothing | Ty::Error) {
        return;
    }
    let args: Vec<String> = inputs.iter().map(print::expr).collect();
    let call = match inputs {
        [] => feature.to_string(),
        _ => format!("{feature} with {}", args.join(", ")),
    };
    let msg = format!(
        "{feature} gives {}; use it, as in set result to {call}",
        ty.described()
    );
    ctx.diags.push(Diagnostic::error(ctx.src, line, msg));
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn results() {
        let p = "feature start\n  show (twice with 2)\nfeature twice\n  needs n as number\n  give n * 2\n";
        assert!(diags(p).is_empty());
        let rec = "feature start\n  show (down with 3)\nfeature down\n  needs n as number\n  if n is 0\n    give 0\n  give down with n - 1\n";
        assert!(diags(rec).is_empty());
        let mutual = "feature start\n  show (a with 1)\nfeature a\n  needs n as number\n  give b with n\nfeature b\n  needs n as number\n  give a with n\n";
        assert_eq!(
            diags(mutual),
            [
                "line 3: feature a. cannot work out what a gives; add a give that does not call a.",
                "line 6: feature b. cannot work out what b gives; add a give that does not call b.",
            ]
        );
    }

    #[test]
    fn a_cycle_is_reported_beside_unrelated_errors() {
        let p = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  show n + \"a\"\n  give f with n\n";
        assert_eq!(
            diags(p),
            [
                "line 3: feature f. cannot work out what f gives; add a give that does not call f.",
                "line 5: show n + \"a\". + needs numbers on both sides; \"a\" is text."
            ]
        );
        let own = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  give f with n + \"a\"\n";
        assert_eq!(diags(own).len(), 1);
    }

    #[test]
    fn a_feature_waiting_on_a_broken_feature_is_not_a_cycle() {
        let p = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  give g with n\nfeature g\n  needs n as number\n  give n + \"a\"\n";
        assert_eq!(
            diags(p),
            ["line 8: give n + \"a\". + needs numbers on both sides; \"a\" is text."]
        );
        let late = "feature start\n  show (g with 1)\nfeature g\n  needs n as number\n  give n + \"a\"\nfeature f\n  needs n as number\n  give g with n\n";
        assert_eq!(diags(late).len(), 1);
    }

    #[test]
    fn a_cycle_through_a_helper_is_still_reported() {
        let p = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  set x to f with n\n  give x\n";
        assert_eq!(
            diags(p),
            ["line 3: feature f. cannot work out what f gives; add a give that does not call f."]
        );
    }

    #[test]
    fn give_rules() {
        let mixed = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  if n is 1\n    give \"one\"\n  give n\n";
        assert_eq!(
            diags(mixed),
            [
                "line 7: give n. Every give in f must give the same type: line 6 gives text, this line gives a number."
            ]
        );
        let partial = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  if n is 1\n    give 1\n";
        assert_eq!(
            diags(partial),
            ["line 3: feature f. f can reach its end without giving a value."]
        );
        assert_eq!(
            diags("feature start\n  give 1\n"),
            ["line 2: give 1. start cannot give a value; it is where the program begins."]
        );
    }

    #[test]
    fn run_versus_value() {
        let p = "feature start\n  run total with \"food\"\nfeature total\n  needs k as text\n  give 1\n";
        assert_eq!(
            diags(p),
            [
                "line 2: run total with \"food\". total gives a number; use it, as in set result to total with \"food\"."
            ]
        );
        let q = "feature start\n  show report\nfeature report\n  show 1\n";
        assert_eq!(
            diags(q),
            [
                "line 2: show report. report gives nothing, so it cannot be used as a value; use run report."
            ]
        );
        let r = "feature start\n  run nothing_here\n";
        assert_eq!(
            diags(r),
            ["line 2: run nothing_here. There is no feature named nothing_here."]
        );
    }

    #[test]
    fn recursive_give_before_the_base_case_resolves() {
        let p = "feature start\n  show (down with 3)\nfeature down\n  needs n as number\n  if n is 0\n    give down with n\n  give 0\n";
        assert!(diags(p).is_empty());
    }

    #[test]
    fn a_give_inside_a_loop_is_not_on_every_path() {
        let p = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  repeat while n is 1\n    give 1\n";
        assert_eq!(
            diags(p),
            ["line 3: feature f. f can reach its end without giving a value."]
        );
    }

    #[test]
    fn a_trailing_note_after_the_final_give_is_fine() {
        let p = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  give n\n  note done\n";
        assert!(diags(p).is_empty());
    }

    #[test]
    fn run_suggestion_lists_every_input() {
        let p = "feature start\n  run f with 1, 2\nfeature f\n  needs a as number, b as number\n  give a\n";
        assert_eq!(
            diags(p),
            ["line 2: run f with 1, 2. f gives a number; use it, as in set result to f with 1, 2."]
        );
    }
}
