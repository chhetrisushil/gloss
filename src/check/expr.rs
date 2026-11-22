use std::collections::BTreeMap;

use super::{Ctx, Ty, similar};
use crate::ast::{Expr, ExprKind, ItemType, PlainType};
use crate::diag::Diagnostic;

pub struct Env<'a> {
    pub vars: &'a BTreeMap<String, Ty>,
    pub results: &'a BTreeMap<String, Ty>,
}

fn plain(p: PlainType) -> Ty {
    match p {
        PlainType::Number => Ty::Number,
        PlainType::Text => Ty::Text,
        PlainType::Truth => Ty::Truth,
    }
}

fn item_ty(t: &ItemType) -> Ty {
    match t {
        ItemType::Plain(p) => plain(*p),
        ItemType::Record(name) => Ty::Record(name.clone()),
    }
}

fn report(ctx: &mut Ctx, e: &Expr, problem: String, suggest: Option<&str>) -> Ty {
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
        ExprKind::Shown => Ty::List(Box::new(Ty::Text)),
        ExprKind::EmptyList(item) => Ty::List(Box::new(item_ty(item))),
        ExprKind::Paren(inner) => type_of(ctx, env, inner, types),
        _ => Ty::Error,
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
    if let Some(f) = ctx
        .program
        .features()
        .find(|f| f.name == name && f.needs.is_empty())
    {
        return env.results.get(&f.name).cloned().unwrap_or(Ty::Error);
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
        assert!(body("  set n to 1\n  show n, es, \"a\", yes, shown\n").is_empty());
        assert_eq!(
            body("  show totl\n"),
            ["line 6: show totl. totl has no value."]
        );
        assert_eq!(
            body("  set total to 1\n  show total, totl\n"),
            ["line 7: show total, totl. totl has no value. Did you mean total?"]
        );
    }
}
