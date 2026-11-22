use std::collections::BTreeMap;

use super::expr::{self, Env};
use super::{Ctx, FeatureInfo, Ty};
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, Line};
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
}
