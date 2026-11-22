use std::collections::BTreeMap;

use super::expr::{self, Env};
use super::{Ctx, FeatureInfo, Ty};
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, ExprId, ExprKind, Line};
use crate::diag::Diagnostic;

/// The state of walking one body: variables in scope and what made them.
struct Walk<'a, 'b> {
    ctx: &'a mut Ctx<'b>,
    results: &'a BTreeMap<String, Ty>,
    types: &'a mut Vec<Option<Ty>>,
    vars: BTreeMap<String, Ty>,
    made: Vec<(String, Ty, u32)>,
    result: Option<Ty>,
}

/// Types every statement of a feature or test body, in source order.
pub fn check_body(
    ctx: &mut Ctx,
    results: &BTreeMap<String, Ty>,
    inputs: &[(String, Ty)],
    body: &[Stmt],
    types: &mut Vec<Option<Ty>>,
) -> FeatureInfo {
    let vars = inputs.iter().cloned().collect();
    let mut walk = Walk {
        ctx,
        results,
        types,
        vars,
        made: Vec::new(),
        result: None,
    };
    walk.stmts(body);
    FeatureInfo {
        inputs: inputs.to_vec(),
        result: walk.result.unwrap_or(Ty::Nothing),
        vars: walk.made,
    }
}

impl Walk<'_, '_> {
    fn ty(&mut self, e: &Expr) -> Ty {
        let env = Env {
            vars: &self.vars,
            results: self.results,
        };
        expr::type_of(self.ctx, &env, e, self.types)
    }

    fn error(&mut self, line: Line, problem: String) {
        self.ctx
            .diags
            .push(Diagnostic::error(self.ctx.src, line, problem));
    }

    fn stmts(&mut self, body: &[Stmt]) {
        for stmt in body {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Set { target, value } => {
                let ty = self.ty(value);
                if let Target::Var(name) = target {
                    let action = format!("be set to {}", ty.described());
                    self.bind(stmt.line, name, ty, &action);
                }
            }
            StmtKind::Ask { question, into } => {
                self.ty(question);
                self.bind(stmt.line, into, Ty::Text, "be asked into, which gives text");
            }
            StmtKind::Show(items) => items.iter().for_each(|i| {
                self.ty(i);
            }),
            StmtKind::Give(value) => {
                let ty = self.ty(value);
                self.result.get_or_insert(ty);
            }
            StmtKind::Run { feature, inputs } => {
                let args: Vec<(&Expr, Ty)> = inputs.iter().map(|i| (i, self.ty(i))).collect();
                let at = Expr {
                    id: ExprId(u32::MAX),
                    line: stmt.line,
                    kind: ExprKind::Call {
                        feature: feature.clone(),
                        inputs: Vec::new(),
                    },
                };
                let env = Env {
                    vars: &self.vars,
                    results: self.results,
                };
                super::combine::call_type(self.ctx, &env, &at, feature, &args);
            }
            StmtKind::If {
                branches,
                otherwise,
            } => {
                for (cond, body) in branches {
                    self.ty(cond);
                    self.stmts(body);
                }
                self.stmts(otherwise.as_deref().unwrap_or_default());
            }
            StmtKind::RepeatWhile { cond, body } => {
                self.ty(cond);
                self.stmts(body);
            }
            _ => {}
        }
    }

    /// Creates `name` at its first `set` or `ask`; later ones must match its type.
    fn bind(&mut self, line: Line, name: &str, ty: Ty, action: &str) {
        let program = self.ctx.program;
        if program.tables().any(|t| t.name == name) {
            let msg = format!("{name} is a table; a variable cannot reuse its name");
            return self.error(line, msg);
        }
        let Some(have) = self.vars.get(name).cloned() else {
            self.vars.insert(name.into(), ty.clone());
            self.made.push((name.into(), ty, line));
            return;
        };
        if ty == Ty::Error || have == Ty::Error || have == ty {
            return;
        }
        let origin = match self.made.iter().find(|(n, ..)| n == name) {
            Some((.., at)) => format!("line {at} made it {}", have.described()),
            None => "it is an input of this feature".to_string(),
        };
        let msg = format!(
            "{name} is {}, so it cannot {action} ({origin})",
            have.described()
        );
        self.error(line, msg);
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";
    fn start(lines: &str) -> Vec<String> {
        diags(&format!("{HEAD}feature start\n{lines}"))
    }

    // Every variable below is also read, so the unused-variable warning from Task 17 never adds noise.
    #[test]
    fn variables_have_fixed_types() {
        assert_eq!(
            start("  set total to 0\n  set total to \"a\"\n  show total\n"),
            [
                "line 6: set total to \"a\". total is a number, so it cannot be set to text (line 5 made it a number)."
            ]
        );
        assert_eq!(
            start("  ask \"q\" into c\n  set c to 1\n  show c\n"),
            [
                "line 6: set c to 1. c is text, so it cannot be set to a number (line 5 made it text)."
            ]
        );
        assert_eq!(
            start("  set es to 1\n"),
            ["line 5: set es to 1. es is a table; a variable cannot reuse its name."]
        );
    }

    #[test]
    fn gives_runs_and_branches() {
        let f = "feature f\n  needs n as number\n  give n\n";
        let at = |lines: &str| diags(&format!("{HEAD}{f}feature start\n{lines}"));
        assert_eq!(
            at("  set x to f with 1\n  set x to \"a\"\n  show x\n"),
            [
                "line 9: set x to \"a\". x is a number, so it cannot be set to text (line 8 made it a number)."
            ]
        );
        assert_eq!(
            at("  run f\n"),
            ["line 8: run f. f needs 1 input: n; this call gives 0."]
        );
        assert!(at("  if yes\n    set y to 1\n  otherwise\n    set y to 2\n  repeat while no\n    show y\n").is_empty());
    }
}
