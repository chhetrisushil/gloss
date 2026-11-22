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

use crate::ast::ExprId;
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

/// What a name in an expression refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    Variable,
    LoopVariable,
    Input,
    Table,
    /// A feature with no inputs, used by its name.
    FeatureCall,
}

/// A loop variable and the line of the loop that makes it, where its scope starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopVar {
    pub name: String,
    pub ty: Ty,
    pub line: u32,
}

/// What the checker found out about one test body.
pub struct TestInfo {
    pub title: String,
    pub vars: Vec<(String, Ty, u32)>,
    pub loops: Vec<LoopVar>,
}

pub struct FeatureInfo {
    pub inputs: Vec<(String, Ty)>,
    pub result: Ty,
    pub gives: Vec<(u32, Ty)>,
    /// True when every give's type is unknown only because it waits on a feature result
    /// that is itself unknown (a cycle), with no error of its own.
    pub cycle: bool,
    pub vars: Vec<(String, Ty, u32)>,
    pub loops: Vec<LoopVar>,
}

#[derive(Default)]
pub struct TypeInfo {
    pub expr_types: Vec<Option<Ty>>,
    pub features: BTreeMap<String, FeatureInfo>,
    /// What each `Name` expression refers to.
    pub names: BTreeMap<ExprId, NameKind>,
    /// One entry per test, in source order.
    pub tests: Vec<TestInfo>,
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
    pub names: BTreeMap<ExprId, NameKind>,
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
            names: BTreeMap::new(),
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
    let known = flow::Known::of_program(program);
    for f in program.features() {
        let kept = ctx.diags.len();
        let inputs = features::inputs_of(f);
        let checked =
            stmt::check_body(ctx, &results, &inputs, &f.body, false, &mut info.expr_types);
        features::check_result(ctx, f, &checked, kept);
        features::check_gives(ctx, f, &checked);
        flow::check_flow(ctx, &known, &inputs, &f.body);
        rules::check_rules(ctx, &info.expr_types, &f.body);
        mutation::check_mutation(ctx, &inputs, &f.body);
        info.features.insert(f.name.clone(), checked);
    }
    for t in program.tests() {
        let checked = stmt::check_body(ctx, &results, &[], &t.body, true, &mut info.expr_types);
        info.tests.push(TestInfo {
            title: t.title.clone(),
            vars: checked.vars,
            loops: checked.loops,
        });
        flow::check_flow(ctx, &known, &[], &t.body);
        rules::check_rules(ctx, &info.expr_types, &t.body);
        mutation::check_mutation(ctx, &[], &t.body);
    }
    walking::check_walking(ctx);
    saving::check_saving(ctx);
    info.names = std::mem::take(&mut ctx.names);
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::stmt::Stmt;
    use crate::ast::{Expr, ExprKind};

    fn names_in(stmts: &[Stmt], out: &mut Vec<(ExprId, String)>) {
        for s in stmts {
            let (mut exprs, mut nested) = (Vec::new(), Vec::new());
            crate::ast::walk::parts(&s.kind, &mut exprs, &mut nested);
            exprs.into_iter().for_each(|e| name_exprs(e, out));
            nested.into_iter().for_each(|b| names_in(b, out));
        }
    }

    fn name_exprs(e: &Expr, out: &mut Vec<(ExprId, String)>) {
        if let ExprKind::Name(n) = &e.kind {
            out.push((e.id, n.clone()));
        }
        crate::ast::walk::children(e)
            .into_iter()
            .for_each(|c| name_exprs(c, out));
    }

    #[test]
    fn type_info_says_what_every_name_in_the_complete_example_refers_to() {
        let text = crate::testing::expenses();
        let src = Source::new(&text);
        let program = crate::parse_source(&src).unwrap();
        let info = check(&program, &src)
            .types
            .expect("the example checks clean");
        let mut found = Vec::new();
        for f in program.features() {
            names_in(&f.body, &mut found);
        }
        for t in program.tests() {
            names_in(&t.body, &mut found);
        }
        assert!(found.len() > 10);
        for (id, name) in &found {
            assert!(info.names.contains_key(id), "no entry for {name}");
        }
        let kinds: Vec<NameKind> = info.names.values().copied().collect();
        for kind in [
            NameKind::Variable,
            NameKind::LoopVariable,
            NameKind::Input,
            NameKind::Table,
        ] {
            assert!(kinds.contains(&kind), "no name is a {kind:?}");
        }
        let total = found.iter().find(|(_, n)| n == "total").unwrap();
        assert_eq!(info.names[&total.0], NameKind::Variable);
        assert_eq!(info.tests.len(), program.tests().count());
        let looped: Vec<&str> = info.features["category_total"]
            .loops
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        assert_eq!(looped, ["expense"]);
        assert!(
            info.features["category_total"]
                .loops
                .iter()
                .all(|l| l.line > 0)
        );
    }

    #[test]
    fn type_info_has_feature_calls_and_test_variables() {
        let text = "feature one\n  give 1\nfeature start\n  show one\ntest \"t\"\n  set x to one\n  for each i from 1 to 2\n    show i, x\n";
        let src = Source::new(text);
        let program = crate::parse_source(&src).unwrap();
        let info = check(&program, &src).types.expect("checks clean");
        assert!(info.names.values().any(|k| *k == NameKind::FeatureCall));
        let test = &info.tests[0];
        assert_eq!(test.title, "t");
        assert_eq!(test.vars, [("x".to_string(), Ty::Number, 6)]);
        assert_eq!(
            test.loops,
            [LoopVar {
                name: "i".into(),
                ty: Ty::Number,
                line: 7
            }]
        );
    }
}
