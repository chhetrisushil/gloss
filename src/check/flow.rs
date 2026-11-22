use std::collections::{BTreeMap, BTreeSet};

use super::uses::{self, Origin};
use super::{Ctx, Ty};
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, Line};
use crate::diag::Diagnostic;

struct Flow<'a, 'b> {
    ctx: &'a mut Ctx<'b>,
    origins: BTreeMap<String, Origin>,
}

type Defined = BTreeSet<String>;

/// Reports every read of a variable that may not have a value yet.
pub fn check_flow(ctx: &mut Ctx, inputs: &[(String, Ty)], body: &[Stmt]) {
    let mut found = Vec::new();
    uses::firsts(body, None, &mut found);
    let mut flow = Flow {
        ctx,
        origins: found.into_iter().collect(),
    };
    let mut defined: Defined = inputs.iter().map(|(n, _)| n.clone()).collect();
    flow.stmts(body, &mut defined);
}

impl Flow<'_, '_> {
    fn read_name(&mut self, line: Line, name: &str, defined: &mut Defined) {
        let Some(origin) = self.origins.get(name) else {
            return;
        };
        if !defined.insert(name.into()) {
            return;
        }
        let (first, fix) = origin.describe();
        let msg =
            format!("{name} may not have a value here; it is first set {first}. Set it {fix}");
        self.ctx
            .diags
            .push(Diagnostic::error(self.ctx.src, line, msg));
    }

    fn read(&mut self, line: Line, e: &Expr, defined: &mut Defined) {
        let mut names = Vec::new();
        uses::reads(e, &mut names);
        for name in names {
            self.read_name(line, name, defined);
        }
    }

    /// A field or item target changes part of a value, so it reads the root variable.
    fn read_target(&mut self, line: Line, t: &Target, defined: &mut Defined) {
        match t {
            Target::Var(name) => self.read_name(line, name, defined),
            Target::Field { of, .. } => self.read_target(line, of, defined),
            Target::Item { position, of } => {
                self.read(line, position, defined);
                self.read_target(line, of, defined);
            }
        }
    }

    fn stmts(&mut self, body: &[Stmt], defined: &mut Defined) {
        for stmt in body {
            self.stmt(stmt, defined);
        }
    }

    fn block(&mut self, body: &[Stmt], defined: &Defined, extra: Option<&str>) -> Defined {
        let mut inner = defined.clone();
        inner.extend(extra.map(String::from));
        self.stmts(body, &mut inner);
        inner
    }

    fn stmt(&mut self, stmt: &Stmt, defined: &mut Defined) {
        let line = stmt.line;
        match &stmt.kind {
            StmtKind::Set {
                target: Target::Var(name),
                value,
            } => {
                self.read(line, value, defined);
                defined.insert(name.clone());
            }
            StmtKind::Set { target, value } => {
                self.read(line, value, defined);
                self.read_target(line, target, defined);
            }
            StmtKind::Add { value: e, target }
            | StmtKind::Subtract { value: e, target }
            | StmtKind::Multiply { target, by: e }
            | StmtKind::Divide { target, by: e } => {
                self.read(line, e, defined);
                self.read_target(line, target, defined);
            }
            StmtKind::Append { value, list } => {
                self.read(line, value, defined);
                self.read_name(line, list, defined);
            }
            StmtKind::Ask { question, into } => {
                self.read(line, question, defined);
                defined.insert(into.clone());
            }
            StmtKind::Show(xs) | StmtKind::Input(xs) => {
                xs.iter().for_each(|x| self.read(line, x, defined));
            }
            StmtKind::Run { inputs, .. } => inputs.iter().for_each(|x| self.read(line, x, defined)),
            StmtKind::Give(e) | StmtKind::Expect(e) => self.read(line, e, defined),
            StmtKind::If {
                branches,
                otherwise,
            } => {
                let mut live = Vec::new();
                for (cond, b) in branches {
                    self.read(cond.line, cond, defined);
                    let reached = self.block(b, defined, None);
                    live.extend((!uses::diverges(b)).then_some(reached));
                }
                if let Some(b) = otherwise {
                    live.extend((!uses::diverges(b)).then(|| self.block(b, defined, None)));
                    let mut sets = live.into_iter();
                    match sets.next() {
                        Some(first) => defined.extend(sets.fold(first, |a, s| &a & &s)),
                        None => defined.extend(self.origins.keys().cloned()),
                    }
                }
            }
            StmtKind::ForEachIn { var, list, body } => {
                self.read(line, list, defined);
                self.block(body, defined, Some(var));
            }
            StmtKind::ForEachRange {
                var,
                from,
                to,
                body,
            } => {
                self.read(line, from, defined);
                self.read(line, to, defined);
                self.block(body, defined, Some(var));
            }
            StmtKind::RepeatWhile { cond, body } => {
                self.read(line, cond, defined);
                self.block(body, defined, None);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn definite_values() {
        let p = "feature start\n  if 1 is 1\n    set a to 1\n  show a\n";
        assert_eq!(
            diags(p),
            [
                "line 4: show a. a may not have a value here; it is first set inside the if on line 2. Set it before the if."
            ]
        );
        let q = "feature start\n  for each i from 1 to 3\n    set last_i to i\n  show last_i\n";
        assert_eq!(
            diags(q),
            [
                "line 4: show last_i. last_i may not have a value here; it is first set inside the loop on line 2. Set it before the loop."
            ]
        );
        let ok =
            "feature start\n  if 1 is 1\n    set a to 1\n  otherwise\n    set a to 2\n  show a\n";
        assert!(diags(ok).is_empty());
    }

    #[test]
    fn diverging_branches_do_not_count() {
        let give = "feature f\n  needs n as number\n  if n is 0\n    give 1\n  otherwise\n    set r to n\n  give r\nfeature start\n  show (f with 1)\n";
        assert!(diags(give).is_empty());
        let stop = "feature start\n  repeat while yes\n    if 1 is 1\n      stop\n    otherwise\n      set r to 1\n    show r\n";
        assert!(diags(stop).is_empty());
        let none = "feature start\n  if 1 is 1\n    set a to 1\n  show a\n";
        assert_eq!(diags(none).len(), 1);
    }

    #[test]
    fn read_earlier_in_the_loop_than_the_set() {
        let p = "feature start\n  repeat while no\n    show a\n    set a to 1\n";
        assert_eq!(
            diags(p),
            [
                "line 3: show a. a may not have a value here; it is first set inside the loop on line 2. Set it before the loop."
            ]
        );
    }
}
