use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::similar::Neighbours;
use super::uses::{self, Exits, Origin};
use super::{Ctx, Ty};
use crate::ast::program::Program;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::walk::for_each_stmt;
use crate::ast::{Expr, Line};
use crate::diag::Diagnostic;

struct Flow<'a, 'b> {
    ctx: &'a mut Ctx<'b>,
    origins: BTreeMap<String, Origin>,
    used: BTreeSet<String>,
    /// Defensive: no input produces a duplicate report for one name and line today.
    reported: BTreeSet<(String, Line)>,
    /// How many loops the statement being walked is inside.
    loops: u32,
}

type Defined = BTreeSet<String>;

/// Reports reads of a variable that may not have a value yet, then variables
/// never read and new names that look like typos or differ only in case.
pub fn check_flow(ctx: &mut Ctx, base: &Known, inputs: &[(String, Ty)], body: &[Stmt]) {
    let mut found = Vec::new();
    uses::firsts(body, None, &mut found);
    // An input already has a value, so setting it makes no new variable.
    found.retain(|(name, _)| {
        ctx.declared_as(name).is_none() && !inputs.iter().any(|(n, _)| n == name)
    });
    let mut flow = Flow {
        ctx,
        origins: found.iter().cloned().collect(),
        used: BTreeSet::new(),
        reported: BTreeSet::new(),
        loops: 0,
    };
    let mut defined: Defined = inputs.iter().map(|(n, _)| n.clone()).collect();
    flow.stmts(body, &mut defined);
    let mut loops = Vec::new();
    loop_vars(body, &mut loops);
    flow.names(base, inputs, &found, &loops);
}

/// Every loop variable in the body, which the typo warning treats as an existing name.
fn loop_vars<'a>(body: &'a [Stmt], out: &mut Vec<&'a str>) {
    for_each_stmt(body, &mut |stmt| {
        if let StmtKind::ForEachIn { var, .. } | StmtKind::ForEachRange { var, .. } = &stmt.kind {
            out.push(var);
        }
    });
}

/// True when the names are the same but for one final `s`.
fn plural(a: &str, b: &str) -> bool {
    let one = |short: &str, long: &str| long.strip_suffix('s') == Some(short);
    one(a, b) || one(b, a)
}

/// The names a new name is compared with, in the order they became known.
#[derive(Default)]
pub struct Known<'a> {
    near: Neighbours<'a>,
    by_lower: HashMap<String, Vec<&'a str>>,
    /// Record names by lowercase; a record clashes with a name unless only the first letter differs.
    records: HashMap<String, Vec<&'a str>>,
}

impl<'a> Known<'a> {
    /// The tables and features of a program, which every body is compared with.
    pub fn of_program(program: &'a Program) -> Self {
        let mut known = Known::default();
        program.tables().for_each(|t| known.add(&t.name));
        program.features().for_each(|f| known.add(&f.name));
        for r in program.records() {
            known
                .records
                .entry(r.name.to_lowercase())
                .or_default()
                .push(&r.name);
        }
        known
    }

    fn add(&mut self, name: &'a str) {
        self.near.add(name);
        self.by_lower
            .entry(name.to_lowercase())
            .or_default()
            .push(name);
    }

    /// The first known name that is `name` in other letter case.
    fn clash(&self, name: &str) -> Option<&'a str> {
        let lower = name.to_lowercase();
        let tail = |s: &str| s.chars().skip(1).collect::<String>();
        let same = self.by_lower.get(&lower).into_iter().flatten().copied();
        let records = self.records.get(&lower).into_iter().flatten().copied();
        let mut others = same.filter(|k| *k != name);
        others
            .next()
            .or_else(|| records.into_iter().find(|r| tail(r) != tail(name)))
    }

    /// The first known name one edit from `name`, if `name` is long enough to look like a typo.
    fn typo(&self, name: &str) -> Option<&'a str> {
        if name.chars().count() < 4 {
            return None;
        }
        let lower = name.to_lowercase();
        let mut near = self.near.within_one(name).into_iter();
        near.find(|k| k.to_lowercase() != lower && !plural(name, k))
    }
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
    fn names(
        &mut self,
        base: &Known,
        inputs: &[(String, Ty)],
        found: &[(String, Origin)],
        loops: &[&str],
    ) {
        let mut own = Known::default();
        inputs.iter().for_each(|(n, _)| own.add(n));
        let mut set = Known::default();
        let mut loop_names = Known::default();
        loops.iter().for_each(|n| loop_names.add(n));
        // The order names became known decides which one a message names.
        for (name, origin) in found {
            let (src, line) = (self.ctx.src, origin.line);
            let clash = [&own, base, &set].iter().find_map(|k| k.clash(name));
            let typo = [&own, base, &set, &loop_names]
                .iter()
                .find_map(|k| k.typo(name));
            if let Some(other) = clash {
                let msg = format!("{name} and {other} differ only in case; rename one");
                self.ctx.diags.push(Diagnostic::error(src, line, msg));
            } else if let Some(near) = typo {
                let msg = format!("new name {name}; did you mean {near}?");
                self.ctx.diags.push(Diagnostic::warning(src, line, msg));
            }
            if !self.used.contains(name) {
                let msg = format!("{name} is set but never used");
                self.ctx.diags.push(Diagnostic::warning(src, line, msg));
            }
            set.add(name);
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
        let mut ended: Option<String> = None;
        for stmt in body {
            if !matches!(stmt.kind, StmtKind::Note(_)) {
                if let Some(cause) = ended.take() {
                    self.never_runs(stmt.line, &cause);
                }
                ended = self.ends_block(stmt);
            }
            self.stmt(stmt, defined);
        }
    }

    /// Why the rest of the block never runs: a give, or a stop or skip inside a loop, or an if
    /// with an otherwise whose every branch ends the feature or the loop.
    fn ends_block(&self, stmt: &Stmt) -> Option<String> {
        let at = stmt.line;
        let loops = self.loops > 0;
        match &stmt.kind {
            StmtKind::Give(_) => Some(format!("the give on line {at} ends the feature")),
            StmtKind::Stop if loops => Some(format!("the stop on line {at} ends the loop")),
            StmtKind::Skip if loops => Some(format!(
                "the skip on line {at} moves on to the next pass of the loop"
            )),
            StmtKind::If {
                branches,
                otherwise: Some(other),
            } => {
                let all = |exits| {
                    let ends = |b: &[Stmt]| uses::diverges(b, exits);
                    branches.iter().all(|(_, b)| ends(b)) && ends(other)
                };
                let end = if all(Exits::Give) {
                    "feature"
                } else if loops && all(Exits::Any) {
                    "loop"
                } else {
                    return None;
                };
                Some(format!(
                    "every branch of the if on line {at} ends the {end}"
                ))
            }
            _ => None,
        }
    }

    fn never_runs(&mut self, line: Line, cause: &str) {
        let msg = format!("this line never runs; {cause}");
        self.ctx
            .diags
            .push(Diagnostic::warning(self.ctx.src, line, msg));
    }

    fn looped(&mut self, body: &[Stmt], defined: &Defined, extra: Option<&str>) {
        self.loops += 1;
        self.block(body, defined, extra);
        self.loops -= 1;
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
                    live.extend((!uses::diverges(b, Exits::Any)).then_some(reached));
                }
                if let Some(b) = otherwise {
                    live.extend(
                        (!uses::diverges(b, Exits::Any)).then(|| self.block(b, defined, None)),
                    );
                    let mut sets = live.into_iter();
                    match sets.next() {
                        Some(first) => defined.extend(sets.fold(first, |a, s| &a & &s)),
                        None => defined.extend(self.origins.keys().cloned()),
                    }
                }
            }
            StmtKind::ForEachIn { var, list, body } => {
                self.read(line, list, defined);
                self.looped(body, defined, Some(var));
            }
            StmtKind::ForEachRange {
                var,
                from,
                to,
                body,
            } => {
                self.read(line, from, defined);
                self.read(line, to, defined);
                self.looped(body, defined, Some(var));
            }
            StmtKind::RepeatWhile { cond, body } => {
                self.read(line, cond, defined);
                self.looped(body, defined, None);
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
    fn a_loop_variable_is_a_name_the_typo_warning_knows() {
        let p = "feature start\n  set xs to list of 1\n  for each total in xs\n    show total\n  set totl to 2\n  show totl\n";
        assert_eq!(
            diags(p),
            ["line 5: warning: set totl to 2. new name totl; did you mean total?"]
        );
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

    #[test]
    fn a_position_in_an_item_target_is_a_read() {
        let used = "feature start\n  set xs to list of 1\n  set i to 1\n  set item i of xs to 1\n  show xs\n";
        assert!(diags(used).is_empty());
        let early = "feature start\n  set xs to list of 1\n  set item i of xs to 1\n  set i to 1\n  show xs\n";
        assert_eq!(
            diags(early),
            [
                "line 3: set item i of xs to 1. i may not have a value here; it is first set on line 4. Set it before this line."
            ]
        );
    }

    #[test]
    fn setting_an_input_is_not_a_new_unused_variable() {
        let p = "feature f\n  needs n as number\n  set n to 5\n  give 1\nfeature start\n  show (f with 1)\n";
        let all = diags(p);
        assert!(all.iter().all(|d| !d.contains("never used")), "{all:?}");
        assert_eq!(all.len(), 1, "{all:?}");
    }
}
