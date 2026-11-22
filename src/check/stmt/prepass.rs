//! The variable pre-pass: each variable gets the type of its first `set` or `ask`, so a body can
//! read a variable above the line that sets it. Variables are typed in dependency order, each
//! statement once; a variable in a cycle of reads settles as unknown. Nothing is reported.

use std::collections::{HashMap, HashSet};

use super::Walk;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, Line};
use crate::check::{Ty, uses};

/// A loop around a first set: its variable and its list (none for a range, which makes numbers).
type Frame<'a> = (&'a str, Option<&'a Expr>);

/// The first `set` (a value) or `ask` (no value, text) of a variable, and the loops around it.
struct Def<'a> {
    name: &'a str,
    line: Line,
    value: Option<&'a Expr>,
    frames: Vec<Frame<'a>>,
}

impl Walk<'_, '_> {
    pub(super) fn type_variables(&mut self, body: &[Stmt]) {
        let mut defs = Vec::new();
        self.defs(body, &mut Vec::new(), &mut HashSet::new(), &mut defs);
        for d in &defs {
            self.made_at.insert(d.name.into(), self.made.len());
            self.made.push((d.name.into(), Ty::Error, d.line));
            self.vars.insert(d.name.into(), Ty::Error);
        }
        let index: HashMap<&str, usize> =
            defs.iter().enumerate().map(|(i, d)| (d.name, i)).collect();
        // The defs each def reads; a loop's own variable is not one of them.
        let needs: Vec<Vec<usize>> = defs
            .iter()
            .map(|d| {
                let mut names = Vec::new();
                for e in d
                    .value
                    .into_iter()
                    .chain(d.frames.iter().filter_map(|f| f.1))
                {
                    uses::reads(e, &mut names);
                }
                let own = |n: &str| d.frames.iter().any(|f| f.0 == n);
                (names.into_iter())
                    .filter_map(|n| index.get(n).filter(|_| !own(n)).copied())
                    .collect()
            })
            .collect();
        // Depth first without recursion: 1 is being typed (a read of it is a cycle), 2 is typed.
        let mut state = vec![0u8; defs.len()];
        for root in 0..defs.len() {
            let mut stack = vec![(root, 0)];
            while let Some((i, at)) = stack.pop() {
                if at == 0 && std::mem::replace(&mut state[i], 1) != 0 {
                    continue;
                }
                if let Some(&j) = needs[i].get(at) {
                    stack.push((i, at + 1));
                    if state[j] == 0 {
                        stack.push((j, 0));
                    }
                } else {
                    self.resolve(&defs[i]);
                    state[i] = 2;
                }
            }
        }
    }

    /// The first `set` or `ask` of each name, in source order, with the loops around it.
    fn defs<'a>(
        &self,
        body: &'a [Stmt],
        frames: &mut Vec<Frame<'a>>,
        seen: &mut HashSet<&'a str>,
        out: &mut Vec<Def<'a>>,
    ) {
        for stmt in body {
            let mut add = |name: &'a str, value| {
                let known = self.ctx.declared_as(name).is_some() || self.vars.contains_key(name);
                if !known && !frames.iter().any(|f| f.0 == name) && seen.insert(name) {
                    let (line, frames) = (stmt.line, frames.clone());
                    out.push(Def {
                        name,
                        line,
                        value,
                        frames,
                    });
                }
            };
            match &stmt.kind {
                StmtKind::Set {
                    target: Target::Var(name),
                    value,
                } => add(name, Some(value)),
                StmtKind::Ask { into, .. } => add(into, None),
                StmtKind::If {
                    branches,
                    otherwise,
                } => {
                    for b in branches.iter().map(|(_, b)| b).chain(otherwise) {
                        self.defs(b, frames, seen, out);
                    }
                }
                StmtKind::RepeatWhile { body, .. } => self.defs(body, frames, seen, out),
                StmtKind::ForEachIn { var, list, body } => {
                    frames.push((var, Some(list)));
                    self.defs(body, frames, seen, out);
                    frames.pop();
                }
                StmtKind::ForEachRange { var, body, .. } => {
                    frames.push((var, None));
                    self.defs(body, frames, seen, out);
                    frames.pop();
                }
                _ => {}
            }
        }
    }

    /// Types one def with its loop variables in scope and records the variable's type.
    fn resolve(&mut self, d: &Def) {
        let mut hidden = Vec::new();
        for &(var, list) in &d.frames {
            let ty = list.map_or(Ty::Number, |l| self.quiet(l).item().unwrap_or(Ty::Error));
            hidden.push((var, self.vars.insert(var.into(), ty)));
        }
        let ty = d.value.map_or(Ty::Text, |v| self.quiet(v)).norm();
        for (var, old) in hidden.into_iter().rev() {
            match old {
                Some(old) => self.vars.insert(var.into(), old),
                None => self.vars.remove(var),
            };
        }
        self.made[self.made_at[d.name]].1 = ty.clone();
        self.vars.insert(d.name.into(), ty);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn variables_that_read_each_other_settle_without_a_type_error() {
        let out = crate::check_source("feature start\n  set a to b\n  set b to a\n  show a, b\n");
        let said: Vec<String> = out.iter().map(|d| d.render()).collect();
        assert_eq!(
            said,
            [
                "line 2: set a to b. b may not have a value here; it is first set on line 3. Set it before this line."
            ]
        );
    }
}
