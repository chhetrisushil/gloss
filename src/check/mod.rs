pub mod combine;
pub mod expr;
pub mod names;
pub mod similar;
pub mod stmt;
mod types;

use std::collections::BTreeMap;

use crate::ast::Type;
use crate::ast::program::Program;
use crate::diag::{self, Diagnostic};
use crate::source::Source;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Number,
    Text,
    Truth,
    Record(String),
    List(Box<Ty>),
    Table { table: String, record: String },
    Nothing,
    Error,
}

pub struct FeatureInfo {
    pub inputs: Vec<(String, Ty)>,
    pub result: Ty,
    pub vars: Vec<(String, Ty, u32)>,
}

#[derive(Default)]
pub struct TypeInfo {
    pub expr_types: Vec<Option<Ty>>,
    pub features: BTreeMap<String, FeatureInfo>,
}

pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub types: Option<TypeInfo>,
}

pub struct Ctx<'a> {
    pub program: &'a Program,
    pub src: &'a Source,
    pub diags: Vec<Diagnostic>,
}

/// Runs every check pass; `types` is `Some` only when no errors were found.
pub fn check(program: &Program, src: &Source) -> CheckResult {
    let mut ctx = Ctx {
        program,
        src,
        diags: Vec::new(),
    };
    names::check_program(&mut ctx);
    let info = check_bodies(&mut ctx);
    diag::sort(&mut ctx.diags);
    let types = (!ctx.diags.iter().any(Diagnostic::is_error)).then_some(info);
    CheckResult {
        diagnostics: ctx.diags,
        types,
    }
}

/// Checks every feature, then every test; a feature not yet typed is silent.
fn check_bodies(ctx: &mut Ctx) -> TypeInfo {
    let program = ctx.program;
    let mut info = TypeInfo::default();
    let mut results = BTreeMap::new();
    for f in program.features() {
        let inputs: Vec<(String, Ty)> = f
            .needs
            .iter()
            .map(|n| {
                let ty = match &n.ty {
                    Type::Item(t) => expr::item_ty(t),
                    Type::List(t) => Ty::List(Box::new(expr::item_ty(t))),
                };
                (n.name.clone(), ty)
            })
            .collect();
        let checked = stmt::check_body(ctx, &results, &inputs, &f.body, &mut info.expr_types);
        results.insert(f.name.clone(), checked.result.clone());
        info.features.insert(f.name.clone(), checked);
    }
    for t in program.tests() {
        stmt::check_body(ctx, &results, &[], &t.body, &mut info.expr_types);
    }
    info
}
