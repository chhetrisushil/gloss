pub mod combine;
pub mod expr;
mod features;
mod flow;
mod mutation;
pub mod names;
mod order;
pub mod rules;
mod saving;
pub mod similar;
pub mod stmt;
mod types;
pub mod uses;
mod walking;

use std::collections::{BTreeMap, HashMap};

use crate::ast::program::{FeatureDecl, Program};
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
    pub gives: Vec<(u32, Ty)>,
    /// True when every give's type is unknown only because it waits on a feature result
    /// that is itself unknown (a cycle), with no error of its own.
    pub cycle: bool,
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
    /// How many times a feature result that is still unknown was looked up.
    pub pending: usize,
    features: HashMap<&'a str, &'a FeatureDecl>,
    declared: HashMap<&'a str, &'static str>,
}

impl<'a> Ctx<'a> {
    pub fn new(program: &'a Program, src: &'a Source) -> Self {
        let mut features = HashMap::new();
        let mut declared = HashMap::new();
        for t in program.tables() {
            declared.entry(t.name.as_str()).or_insert("a table");
        }
        for f in program.features() {
            features.entry(f.name.as_str()).or_insert(f);
            declared.entry(f.name.as_str()).or_insert("a feature");
        }
        for r in program.records() {
            declared.entry(r.name.as_str()).or_insert("a record");
        }
        Ctx {
            program,
            src,
            diags: Vec::new(),
            pending: 0,
            features,
            declared,
        }
    }

    /// The first feature declared with this name.
    pub fn feature(&self, name: &str) -> Option<&'a FeatureDecl> {
        self.features.get(name).copied()
    }

    /// What a name is already used for ("a feature", "a table" or "a record"), if anything.
    pub fn declared_as(&self, name: &str) -> Option<&'static str> {
        self.declared.get(name).copied()
    }
}

/// Runs every check pass; `types` is `Some` only when no errors were found.
pub fn check(program: &Program, src: &Source) -> CheckResult {
    let mut ctx = Ctx::new(program, src);
    names::check_program(&mut ctx);
    let info = check_bodies(&mut ctx);
    diag::sort(&mut ctx.diags);
    let types = (!ctx.diags.iter().any(Diagnostic::is_error)).then_some(info);
    CheckResult {
        diagnostics: ctx.diags,
        types,
    }
}

/// Checks every feature, then every test, with every feature's result known.
fn check_bodies(ctx: &mut Ctx) -> TypeInfo {
    let program = ctx.program;
    let mut info = TypeInfo::default();
    let results = features::infer_results(ctx);
    for f in program.features() {
        let kept = ctx.diags.len();
        let inputs = features::inputs_of(f);
        let checked =
            stmt::check_body(ctx, &results, &inputs, &f.body, false, &mut info.expr_types);
        features::check_result(ctx, f, &checked, kept);
        features::check_gives(ctx, f, &checked);
        flow::check_flow(ctx, &inputs, &f.body);
        rules::check_rules(ctx, &info.expr_types, &f.body);
        mutation::check_mutation(ctx, &inputs, &f.body);
        info.features.insert(f.name.clone(), checked);
    }
    for t in program.tests() {
        stmt::check_body(ctx, &results, &[], &t.body, true, &mut info.expr_types);
        flow::check_flow(ctx, &[], &t.body);
        rules::check_rules(ctx, &info.expr_types, &t.body);
        mutation::check_mutation(ctx, &[], &t.body);
    }
    walking::check_walking(ctx);
    saving::check_saving(ctx);
    info
}
