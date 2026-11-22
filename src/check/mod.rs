pub mod names;
pub mod similar;
mod types;

use std::collections::BTreeMap;

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
    diag::sort(&mut ctx.diags);
    let types = (!ctx.diags.iter().any(Diagnostic::is_error)).then(TypeInfo::default);
    CheckResult {
        diagnostics: ctx.diags,
        types,
    }
}
