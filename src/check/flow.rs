use std::collections::{BTreeMap, BTreeSet};

use super::similar::distance;
use super::uses::{self, Origin};
use super::{Ctx, Ty};
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, Line};
use crate::diag::Diagnostic;

struct Flow<'a, 'b> {
    ctx: &'a mut Ctx<'b>,
    origins: BTreeMap<String, Origin>,
    used: BTreeSet<String>,
    reported: BTreeSet<(String, Line)>,
}

type Defined = BTreeSet<String>;

/// Reports reads of a variable that may not have a value yet, then variables
/// never read and new names that look like typos or differ only in case.
pub fn check_flow(ctx: &mut Ctx, inputs: &[(String, Ty)], body: &[Stmt]) {
    let mut found = Vec::new();
    uses::firsts(body, None, &mut found);
    found.retain(|(name, _)| !ctx.program.tables().any(|t| &t.name == name));
    let mut flow = Flow {
        ctx,
        origins: found.iter().cloned().collect(),
        used: BTreeSet::new(),
        reported: BTreeSet::new(),
    };
    let mut defined: Defined = inputs.iter().map(|(n, _)| n.clone()).collect();
    flow.stmts(body, &mut defined);
    flow.names(inputs, &found);
}

/// True when the names are the same but for one final `s`.
fn plural(a: &str, b: &str) -> bool {
    let one = |short: &str, long: &str| long.strip_suffix('s') == Some(short);
    one(a, b) || one(b, a)
}

impl Flow<'_, '_> {
    fn read_name(&mut self, line: Line, name: &str, defined: &mut Defined) {
        self.used.insert(name.into());
        let Some(origin) = self.origins.get(name) else {
            return;
        };
        if !defined.insert(name.into()) {
            return;
        }
        if !self.reported.insert((name.into(), line)) {
            return;
        }
        let (first, fix) = origin.describe();
        let msg =
            format!("{name} may not have a value here; it is first set {first}. Set it {fix}");
        self.ctx
            .diags
            .push(Diagnostic::error(self.ctx.src, line, msg));
    }

    /// Warns about unused variables and look-alike new names, and rejects case clashes.
    fn names(&mut self, inputs: &[(String, Ty)], found: &[(String, Origin)]) {
        let program = self.ctx.program;
        let mut known: Vec<&str> = inputs.iter().map(|(n, _)| n.as_str()).collect();
        known.extend(program.tables().map(|t| t.name.as_str()));
        known.extend(program.features().map(|f| f.name.as_str()));
        for (name, origin) in found {
            let (src, line) = (self.ctx.src, origin.line);
            let lower = name.to_lowercase();
            let clash = known
                .iter()
                .find(|k| k.to_lowercase() == lower && *k != name);
            let typo = known
                .iter()
                .find(|k| distance(name, k) == 1 && k.to_lowercase() != lower && !plural(name, k));
            if let Some(other) = clash {
                let msg = format!("{name} and {other} differ only in case; rename one");
                self.ctx.diags.push(Diagnostic::error(src, line, msg));
            } else if let Some(near) = typo.filter(|_| name.chars().count() >= 4) {
                let msg = format!("new name {name}; did you mean {near}?");
                self.ctx.diags.push(Diagnostic::warning(src, line, msg));
            }
            if !self.used.contains(name) {
                let msg = format!("{name} is set but never used");
                self.ctx.diags.push(Diagnostic::warning(src, line, msg));
            }
            known.push(name);
        }
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

    #[test]
    fn a_read_is_reported_once_per_line() {
        let p = "feature start\n  if 1 is 1\n    show x\n  otherwise\n    show x\n  show x\n  set x to 1\n";
        let all = diags(p);
        let said = |l: &str| all.iter().filter(|d| d.starts_with(l)).count();
        assert_eq!(
            (said("line 3:"), said("line 5:"), said("line 6:")),
            (1, 1, 0)
        );
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn warnings() {
        assert_eq!(
            diags("feature start\n  set totl to 5\n"),
            ["line 2: warning: set totl to 5. totl is set but never used."]
        );
        let typo = "feature start\n  set total to 1\n  set totl to 2\n  show total, totl\n";
        assert_eq!(
            diags(typo),
            ["line 3: warning: set totl to 2. new name totl; did you mean total?"]
        );
        let plural = "feature start\n  set pen to 1\n  set pens to list of 2\n  show pen, pens\n";
        assert!(diags(plural).is_empty());
        let short = "feature start\n  set ab to 1\n  set ac to 2\n  show ab, ac\n";
        assert!(diags(short).is_empty());
        let case =
            "feature start\n  set myTotal to 1\n  set mytotal to 2\n  show myTotal, mytotal\n";
        assert_eq!(
            diags(case),
            ["line 3: set mytotal to 2. mytotal and myTotal differ only in case; rename one."]
        );
    }
}
