use std::collections::BTreeMap;

use super::expr::item_ty;
use super::{Ctx, FeatureInfo, Ty, stmt};
use crate::ast::Type;
use crate::ast::program::FeatureDecl;
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
}
