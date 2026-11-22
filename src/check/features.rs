use std::collections::BTreeMap;

use super::expr::item_ty;
use super::{Ctx, FeatureInfo, Ty, stmt};
use crate::ast::Type;
use crate::ast::program::FeatureDecl;
use crate::ast::stmt::{Stmt, StmtKind};
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
    for _ in 0..=results.len() {
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
            break;
        }
    }
    results
}

/// Reports a feature whose result is still unknown with nothing else wrong.
pub fn check_result(ctx: &mut Ctx, f: &FeatureDecl, checked: &FeatureInfo, kept: usize) {
    if checked.result == Ty::Error && ctx.diags.len() == kept {
        let name = &f.name;
        let msg =
            format!("cannot work out what {name} gives; add a give that does not call {name}");
        ctx.diags.push(Diagnostic::error(ctx.src, f.line, msg));
    }
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
}
