use std::collections::BTreeMap;

use super::expr::{self, Env};
use super::rules::target_text;
use super::{Ctx, FeatureInfo, LoopVar, Ty, similar};
use crate::ast::print;
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
    /// Where each name is in `made`.
    made_at: BTreeMap<String, usize>,
    gives: Vec<(Line, Ty)>,
    /// Gives whose type is unknown only because of a still-unknown feature result.
    waiting: usize,
    in_test: bool,
    loops: u32,
    /// The names of the loops being walked; changing one is reported by the mutation check.
    loop_vars: Vec<String>,
    /// The names of the feature's inputs.
    input_names: Vec<String>,
    /// Every loop variable walked, with its loop's line.
    loops_seen: Vec<LoopVar>,
    changed: bool,
}

/// Types every statement of a feature or test body, in source order.
pub fn check_body(
    ctx: &mut Ctx,
    results: &BTreeMap<String, Ty>,
    inputs: &[(String, Ty)],
    body: &[Stmt],
    in_test: bool,
    types: &mut Vec<Option<Ty>>,
) -> FeatureInfo {
    let vars = inputs.iter().cloned().collect();
    let mut walk = Walk {
        ctx,
        results,
        types,
        vars,
        made: Vec::new(),
        made_at: BTreeMap::new(),
        gives: Vec::new(),
        waiting: 0,
        in_test,
        loops: 0,
        loop_vars: Vec::new(),
        input_names: inputs.iter().map(|(n, _)| n.clone()).collect(),
        loops_seen: Vec::new(),
        changed: false,
    };
    for _ in 0..=body.len() + 1 {
        walk.changed = false;
        walk.collect(body);
        if !walk.changed {
            break;
        }
    }
    walk.stmts(body);
    let known = walk.gives.iter().find(|(_, t)| *t != Ty::Error);
    let result = match (known, walk.gives.is_empty()) {
        (Some((_, t)), _) => t.clone(),
        (None, true) => Ty::Nothing,
        (None, false) => Ty::Error,
    };
    FeatureInfo {
        inputs: inputs.to_vec(),
        result,
        cycle: !walk.gives.is_empty() && walk.waiting == walk.gives.len(),
        gives: walk.gives,
        vars: walk.made,
        loops: walk.loops_seen,
    }
}

impl Walk<'_, '_> {
    fn ty(&mut self, e: &Expr) -> Ty {
        let env = Env {
            vars: &self.vars,
            results: self.results,
            in_test: self.in_test,
            inputs: &self.input_names,
            loops: &self.loop_vars,
        };
        expr::type_of(self.ctx, &env, e, self.types)
    }

    fn error(&mut self, line: Line, problem: String) {
        self.ctx
            .diags
            .push(Diagnostic::error(self.ctx.src, line, problem));
    }

    /// The type a target names, reporting a root, field or position that does not fit.
    fn target_ty(&mut self, line: Line, target: &Target) -> Ty {
        let program = self.ctx.program;
        match target {
            Target::Var(name) => {
                if let Some(t) = self.vars.get(name) {
                    return t.clone();
                }
                if let Some(t) = program.tables().find(|t| &t.name == name) {
                    let (table, record) = (t.name.clone(), t.record.clone());
                    return Ty::Table { table, record };
                }
                let names = self.vars.keys().map(String::as_str);
                let near = similar::nearest(name, names.chain(program.tables().map(|t| &*t.name)))
                    .map(str::to_string);
                self.report(line, format!("{name} has no value"), near.as_deref())
            }
            Target::Field { field, of } => match self.target_ty(line, of) {
                Ty::Record(rec) => {
                    let decl = program.records().find(|r| r.name == rec);
                    let fields: Vec<_> = decl.into_iter().flat_map(|r| r.fields()).collect();
                    if let Some(f) = fields.iter().find(|f| &f.name == field) {
                        return expr::plain(f.ty);
                    }
                    let near = similar::nearest(field, fields.iter().map(|f| f.name.as_str()));
                    self.report(line, format!("{rec} has no field {field}"), near)
                }
                Ty::Error => Ty::Error,
                other => {
                    let msg = format!(
                        "{} is {}, so it has no field {field}",
                        target_text(of),
                        other.described()
                    );
                    self.report(line, msg, None)
                }
            },
            Target::Item { position, of } => {
                let p = self.ty(position);
                if !matches!(p, Ty::Number | Ty::Error) {
                    let msg = format!(
                        "The position in item must be a number; {} is {}",
                        print::expr(position),
                        p.described()
                    );
                    self.report(line, msg, None);
                }
                match self.target_ty(line, of) {
                    Ty::Error => Ty::Error,
                    t => t.item().unwrap_or_else(|| {
                        let msg = format!(
                            "item works on lists and tables; {} is {}",
                            target_text(of),
                            t.described()
                        );
                        self.report(line, msg, None)
                    }),
                }
            }
        }
    }

    fn report(&mut self, line: Line, problem: String, suggest: Option<&str>) -> Ty {
        let mut d = Diagnostic::error(self.ctx.src, line, problem);
        if let Some(s) = suggest {
            d = d.suggest(s);
        }
        self.ctx.diags.push(d);
        Ty::Error
    }

    /// `add`, `subtract`, `multiply` and `divide`: a number target and a number value.
    fn arithmetic(&mut self, line: Line, verb: &str, target: &Target, value: &Expr) {
        let v = self.ty(value);
        if !matches!(v, Ty::Number | Ty::Error) {
            let msg = format!(
                "{verb} works on numbers; {} is {}",
                print::expr(value),
                v.described()
            );
            self.report(line, msg, None);
        }
        let t = self.target_ty(line, target);
        let name = target_text(target);
        match (&t, target) {
            (Ty::Number | Ty::Error, _) => {}
            (_, Target::Var(_)) if verb == "add" && t.item().is_some() => {
                let msg = format!(
                    "add works on numbers; to put an item at the end of {name}, use append"
                );
                self.report(
                    line,
                    msg,
                    Some(&format!("append {} to {name}", print::expr(value))),
                );
            }
            _ => {
                let msg = format!("{verb} works on numbers; {name} is {}", t.described());
                self.report(line, msg, None);
            }
        }
    }

    fn stmts(&mut self, body: &[Stmt]) {
        for stmt in body {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Set { target, value } => {
                let ty = self.ty(value).norm();
                if let Target::Var(name) = target {
                    let action = format!("be set to {}", ty.described());
                    self.bind(stmt.line, name, ty, &action);
                } else {
                    let have = self.target_ty(stmt.line, target);
                    if have != Ty::Error && ty != Ty::Error && have != ty {
                        let msg = format!(
                            "{} is {}, so it cannot be set to {}",
                            target_text(target),
                            have.described(),
                            ty.described()
                        );
                        self.error(stmt.line, msg);
                    }
                }
            }
            StmtKind::Add { value, target } => self.arithmetic(stmt.line, "add", target, value),
            StmtKind::Subtract { value, target } => {
                self.arithmetic(stmt.line, "subtract", target, value);
            }
            StmtKind::Multiply { target, by } => {
                self.arithmetic(stmt.line, "multiply", target, by);
            }
            StmtKind::Divide { target, by } => self.arithmetic(stmt.line, "divide", target, by),
            StmtKind::Ask { question, into } => {
                self.require(stmt.line, question, &Ty::Text, "The question must be text");
                self.bind(stmt.line, into, Ty::Text, "be asked into, which gives text");
            }
            StmtKind::Show(items) => items.iter().for_each(|i| {
                self.ty(i);
            }),
            StmtKind::Give(value) => {
                if self.in_test {
                    self.error(stmt.line, "give can only be used inside a feature".into());
                }
                let (kept, pending) = (self.ctx.diags.len(), self.ctx.pending);
                let ty = self.ty(value);
                let own_error = self.ctx.diags.len() > kept;
                if ty == Ty::Error && !own_error && self.ctx.pending > pending {
                    self.waiting += 1;
                }
                self.gives.push((stmt.line, ty.norm()));
            }
            StmtKind::Run { feature, inputs } => {
                let args: Vec<(&Expr, Ty)> = inputs.iter().map(|i| (i, self.ty(i))).collect();
                let kept = self.ctx.diags.len();
                let ty =
                    super::combine::call_type(self.ctx, self.results, stmt.line, feature, &args);
                if self.ctx.diags.len() == kept {
                    super::features::check_run(self.ctx, stmt.line, feature, inputs, &ty);
                }
            }
            StmtKind::If {
                branches,
                otherwise,
            } => {
                for (cond, body) in branches {
                    self.condition(cond.line, cond, "if");
                    self.stmts(body);
                }
                self.stmts(otherwise.as_deref().unwrap_or_default());
            }
            StmtKind::RepeatWhile { cond, body } => {
                self.condition(stmt.line, cond, "repeat while");
                self.in_loop(body);
            }
            StmtKind::Append { value, list } => self.append(stmt.line, value, list),
            StmtKind::Load(name) | StmtKind::Save(name) | StmtKind::Clear(name) => {
                let verb = match &stmt.kind {
                    StmtKind::Load(_) => "load",
                    StmtKind::Save(_) => "save",
                    _ => "clear",
                };
                let t = self.target_ty(stmt.line, &Target::Var(name.clone()));
                if !matches!(t, Ty::Table { .. } | Ty::Error) {
                    let msg = format!("{verb} works on tables; {name} is {}", t.described());
                    self.error(stmt.line, msg);
                }
            }
            StmtKind::ForEachIn { var, list, body } => {
                let t = self.ty(list);
                let item = t.item().unwrap_or_else(|| {
                    if t != Ty::Error {
                        let what =
                            format!("for each … in walks a list or table; {}", print::expr(list));
                        self.error(stmt.line, format!("{what} is {}", t.described()));
                    }
                    Ty::Error
                });
                self.looped(stmt.line, var, item, body);
            }
            StmtKind::ForEachRange {
                var,
                from,
                to,
                body,
            } => {
                for bound in [from, to] {
                    self.require(
                        stmt.line,
                        bound,
                        &Ty::Number,
                        "for each … from walks numbers",
                    );
                }
                self.looped(stmt.line, var, Ty::Number, body);
            }
            StmtKind::Skip | StmtKind::Stop if self.loops == 0 => {
                let word = if stmt.kind == StmtKind::Skip {
                    "skip"
                } else {
                    "stop"
                };
                self.error(stmt.line, format!("{word} can only be used inside a loop"));
            }
            StmtKind::Expect(cond) => {
                self.test_only(stmt.line, "expect");
                self.condition(stmt.line, cond, "expect");
            }
            StmtKind::Input(items) => {
                self.test_only(stmt.line, "input");
                items.iter().for_each(|i| {
                    self.ty(i);
                });
            }
            _ => {}
        }
    }

    fn in_loop(&mut self, body: &[Stmt]) {
        self.loops += 1;
        self.stmts(body);
        self.loops -= 1;
    }

    fn test_only(&mut self, line: Line, word: &str) {
        if !self.in_test {
            self.error(line, format!("{word} can only be used inside a test"));
        }
    }

    /// Types `e` and reports unless it is `want` (or already an error).
    fn require(&mut self, line: Line, e: &Expr, want: &Ty, lead: &str) {
        let t = self.ty(e);
        if t != *want && t != Ty::Error {
            let msg = format!("{lead}; {} is {}", print::expr(e), t.described());
            self.error(line, msg);
        }
    }

    fn condition(&mut self, line: Line, cond: &Expr, word: &str) {
        let lead = format!("The condition of {word} must be yes or no");
        self.require(line, cond, &Ty::Truth, &lead);
    }

    /// Walks a loop body with its variable in scope, then puts back what it hid.
    fn looped(&mut self, line: Line, var: &str, ty: Ty, body: &[Stmt]) {
        let reused = self.ctx.declared_as(var);
        if let Some(noun) = reused {
            let msg = format!("{var} is {noun}; a variable cannot reuse its name");
            self.error(line, msg);
        }
        let clash = self.vars.contains_key(var);
        if clash && reused.is_none() {
            let msg = format!(
                "{var} is already a variable in this feature; pick another name for the loop"
            );
            self.error(line, msg);
        }
        let ty = if clash || reused.is_some() {
            Ty::Error
        } else {
            ty
        };
        self.loops_seen.push(LoopVar {
            name: var.into(),
            ty: ty.clone(),
            line,
        });
        let hidden = self.vars.insert(var.into(), ty);
        self.loop_vars.push(var.into());
        self.in_loop(body);
        self.loop_vars.pop();
        match hidden {
            Some(old) => self.vars.insert(var.into(), old),
            None => self.vars.remove(var),
        };
    }

    fn append(&mut self, line: Line, value: &Expr, list: &str) {
        if self.loop_vars.iter().any(|v| v == list) {
            return;
        }
        let v = self.ty(value);
        let t = self.target_ty(line, &Target::Var(list.into()));
        let Some(item) = t.item() else {
            if t != Ty::Error {
                let msg = format!(
                    "append works on lists and tables; {list} is {}",
                    t.described()
                );
                self.error(line, msg);
            }
            return;
        };
        if v != Ty::Error && v != item {
            let msg = format!(
                "{list} holds {}; {} is {}",
                item.describe(),
                print::expr(value),
                v.described()
            );
            self.error(line, msg);
        }
    }

    /// Pre-pass: each variable gets the type of its first `set` or `ask`, so a
    /// body can read a variable above the line that sets it. Loop variables
    /// are in scope only while their body is collected. Reports nothing.
    fn collect(&mut self, body: &[Stmt]) {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Set {
                    target: Target::Var(name),
                    value,
                } => {
                    let ty = self.quiet(value);
                    self.first(stmt.line, name, ty);
                }
                StmtKind::Ask { into, .. } => self.first(stmt.line, into, Ty::Text),
                StmtKind::If {
                    branches,
                    otherwise,
                } => {
                    branches.iter().for_each(|(_, b)| self.collect(b));
                    self.collect(otherwise.as_deref().unwrap_or_default());
                }
                StmtKind::RepeatWhile { body, .. } => self.collect(body),
                StmtKind::ForEachIn { var, list, body } => {
                    let item = self.quiet(list).item().unwrap_or(Ty::Error);
                    self.scoped(var, item, body);
                }
                StmtKind::ForEachRange { var, body, .. } => self.scoped(var, Ty::Number, body),
                _ => {}
            }
        }
    }

    fn quiet(&mut self, e: &Expr) -> Ty {
        let kept = self.ctx.diags.len();
        let ty = self.ty(e);
        self.ctx.diags.truncate(kept);
        ty
    }

    /// Registers a variable at its first `set` or `ask`. A type that was an
    /// error (it read a variable not yet collected) is replaced once known, but
    /// only by the statement that made the variable.
    fn first(&mut self, line: Line, name: &str, ty: Ty) {
        let ty = ty.norm();
        if self.ctx.declared_as(name).is_some() {
            return;
        }
        match self.made_at.get(name).map(|&i| &mut self.made[i]) {
            None if self.vars.contains_key(name) => {}
            None => {
                self.vars.insert(name.into(), ty.clone());
                self.made_at.insert(name.into(), self.made.len());
                self.made.push((name.into(), ty, line));
                self.changed = true;
            }
            Some((_, have, at)) if *at == line && *have == Ty::Error && ty != Ty::Error => {
                *have = ty.clone();
                self.vars.insert(name.into(), ty);
                self.changed = true;
            }
            Some(_) => {}
        }
    }

    fn scoped(&mut self, var: &str, ty: Ty, body: &[Stmt]) {
        let hidden = self.vars.insert(var.into(), ty);
        self.collect(body);
        match hidden {
            Some(old) => self.vars.insert(var.into(), old),
            None => self.vars.remove(var),
        };
    }

    /// Creates `name` at its first `set` or `ask`; later ones must match its type.
    fn bind(&mut self, line: Line, name: &str, ty: Ty, action: &str) {
        if let Some(noun) = self.ctx.declared_as(name) {
            let msg = format!("{name} is {noun}; a variable cannot reuse its name");
            return self.error(line, msg);
        }
        let Some(have) = self.vars.get(name).cloned() else {
            return;
        };
        if ty == Ty::Error || have == Ty::Error || have == ty {
            return;
        }
        // Inputs and loop variables are reported by the mutation check, once.
        let Some(&(.., at)) = self.made_at.get(name).map(|&i| &self.made[i]) else {
            return;
        };
        if self.loop_vars.iter().any(|v| v == name) {
            return;
        }
        let origin = format!("line {at} made it {}", have.described());
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

    #[test]
    fn arithmetic_verbs_and_targets() {
        assert_eq!(
            start("  set xs to list of 1\n  add 5 to xs\n  show count of xs\n"),
            [
                "line 6: add 5 to xs. add works on numbers; to put an item at the end of xs, use append. Did you mean append 5 to xs?"
            ]
        );
        assert_eq!(
            start("  set n to 1\n  subtract \"a\" from n\n  multiply n by 2\n  show n\n"),
            ["line 6: subtract \"a\" from n. subtract works on numbers; \"a\" is text."]
        );
        assert_eq!(
            start("  set amount of item 1 of es to \"a\"\n  divide amout of item 1 of es by 2\n"),
            [
                "line 5: set amount of item 1 of es to \"a\". amount of item 1 of es is a number, so it cannot be set to text.",
                "line 6: divide amout of item 1 of es by 2. E has no field amout. Did you mean amount?"
            ]
        );
    }

    #[test]
    fn verbs_match_types() {
        assert_eq!(
            start("  set xs to list of 1\n  add 5 to xs\n  show count of xs\n"),
            [
                "line 6: add 5 to xs. add works on numbers; to put an item at the end of xs, use append. Did you mean append 5 to xs?"
            ]
        );
        assert_eq!(
            start("  set n to 1\n  append 5 to n\n  show n\n"),
            ["line 6: append 5 to n. append works on lists and tables; n is a number."]
        );
        assert_eq!(
            start("  append 5 to es\n"),
            ["line 5: append 5 to es. es holds E; 5 is a number."]
        );
        assert_eq!(
            start("  set n to 1\n  load n\n  show n\n"),
            ["line 6: load n. load works on tables; n is a number."]
        );
        assert_eq!(
            start("  if 1\n    show 1\n"),
            ["line 5: if 1. The condition of if must be yes or no; 1 is a number."]
        );
        assert_eq!(
            start("  for each x in 3\n    show x\n"),
            ["line 5: for each x in 3. for each … in walks a list or table; 3 is a number."]
        );
        assert_eq!(
            start("  ask 3 into c\n  show c\n"),
            ["line 5: ask 3 into c. The question must be text; 3 is a number."]
        );
        assert!(start("  load es\n  for each e in es\n    show amount of e\n  for each e from 1 to 3\n    show e\n  save es\n").is_empty());
    }

    #[test]
    fn branches_run_and_save_are_checked() {
        assert_eq!(
            start("  if yes\n    show 1\n  otherwise if 1\n    show 2\n"),
            ["line 7: otherwise if 1. The condition of if must be yes or no; 1 is a number."]
        );
        assert_eq!(
            start("  if yes\n    set v to 1\n  otherwise\n    set v to \"a\"\n  show v\n"),
            [
                "line 8: set v to \"a\". v is a number, so it cannot be set to text (line 6 made it a number)."
            ]
        );
        assert_eq!(
            diags(&format!(
                "{HEAD}feature f\n  needs n as number\n  give n\nfeature start\n  run f with \"a\"\n"
            )),
            ["line 8: run f with \"a\". The input n of f is a number; \"a\" is text."]
        );
        assert_eq!(
            start("  set n to 1\n  save n\n  show n\n"),
            ["line 6: save n. save works on tables; n is a number."]
        );
    }

    #[test]
    fn placement_rules() {
        assert_eq!(
            start("  skip\n"),
            ["line 5: skip. skip can only be used inside a loop."]
        );
        assert_eq!(
            start("  expect 1 is 1\n"),
            ["line 5: expect 1 is 1. expect can only be used inside a test."]
        );
        assert_eq!(
            start("  input \"a\"\n"),
            ["line 5: input \"a\". input can only be used inside a test."]
        );
        assert_eq!(
            start("  show count of shown\n"),
            ["line 5: show count of shown. shown can only be used inside a test."]
        );
        assert_eq!(
            start("  set x to 1\n  show x\n  for each x in es\n    show 1\n"),
            [
                "line 7: for each x in es. x is already a variable in this feature; pick another name for the loop."
            ]
        );
        assert!(
            start(
                "  for each x in es\n    show amount of x\n  for each x from 1 to 3\n    show x\n"
            )
            .is_empty()
        );
        let test = format!(
            "{HEAD}feature start\n  show 1\ntest \"t\"\n  input \"a\"\n  expect count of shown is 0\n"
        );
        assert!(diags(&test).is_empty());
        assert_eq!(
            start("  repeat while no\n    stop\n  stop\n"),
            ["line 7: stop. stop can only be used inside a loop."]
        );
    }

    #[test]
    fn variables_can_be_read_above_their_first_set() {
        assert_eq!(
            diags("feature start\n  show a\n  set a to 1\n"),
            [
                "line 2: show a. a may not have a value here; it is first set on line 3. Set it before this line."
            ]
        );
        assert_eq!(
            diags("feature start\n  show b\n  set a to 1\n  show a\n"),
            ["line 2: show b. b has no value."]
        );
        assert_eq!(
            start("  for each e in es\n    set t to amount of e\n  show t\n"),
            [
                "line 7: show t. t may not have a value here; it is first set inside the loop on line 5. Set it before the loop."
            ]
        );
    }

    #[test]
    fn pre_pass_resolves_forward_reads() {
        assert_eq!(
            start("  set a to b\n  set b to 1\n  set a to \"x\"\n  show a, b\n"),
            [
                "line 5: set a to b. b may not have a value here; it is first set on line 6. Set it before this line.",
                "line 7: set a to \"x\". a is a number, so it cannot be set to text (line 5 made it a number)."
            ]
        );
    }
}
