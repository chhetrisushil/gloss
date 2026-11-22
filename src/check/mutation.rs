use super::{Ctx, Ty};
use crate::ast::print;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::diag::Diagnostic;

/// What a loop variable stands for: an item of a list, or a count.
enum Loop<'a> {
    Item(&'a str, String),
    Count(&'a str),
}

impl Loop<'_> {
    fn name(&self) -> &str {
        match self {
            Loop::Item(name, _) | Loop::Count(name) => name,
        }
    }
}

struct Mutation<'a, 'b, 'c> {
    ctx: &'a mut Ctx<'b>,
    inputs: &'c [(String, Ty)],
}

/// Rejects changes to loop variables and to a feature's inputs.
pub fn check_mutation(ctx: &mut Ctx, inputs: &[(String, Ty)], body: &[Stmt]) {
    Mutation { ctx, inputs }.stmts(body, &mut Vec::new());
}

/// The variable a statement changes, and whether it replaces the whole variable.
fn changed(kind: &StmtKind) -> Option<(&str, bool)> {
    match kind {
        StmtKind::Set { target, .. }
        | StmtKind::Add { target, .. }
        | StmtKind::Subtract { target, .. }
        | StmtKind::Multiply { target, .. }
        | StmtKind::Divide { target, .. } => {
            Some((target.root(), matches!(target, Target::Var(_))))
        }
        StmtKind::Append { list, .. } => Some((list, true)),
        StmtKind::Ask { into, .. } => Some((into, true)),
        _ => None,
    }
}

impl<'a> Mutation<'_, '_, '_> {
    fn stmts(&mut self, body: &'a [Stmt], walking: &mut Vec<Loop<'a>>) {
        for stmt in body {
            self.stmt(stmt, walking);
        }
    }

    fn stmt(&mut self, stmt: &'a Stmt, walking: &mut Vec<Loop<'a>>) {
        if let Some((name, whole)) = changed(&stmt.kind) {
            self.protect(stmt, name, whole, walking);
        }
        match &stmt.kind {
            StmtKind::If {
                branches,
                otherwise,
            } => {
                for (_, b) in branches {
                    self.stmts(b, walking);
                }
                if let Some(b) = otherwise {
                    self.stmts(b, walking);
                }
            }
            StmtKind::ForEachIn { var, list, body } => {
                walking.push(Loop::Item(var, print::expr(list)));
                self.stmts(body, walking);
                walking.pop();
            }
            StmtKind::ForEachRange { var, body, .. } => {
                walking.push(Loop::Count(var));
                self.stmts(body, walking);
                walking.pop();
            }
            StmtKind::RepeatWhile { body, .. } => self.stmts(body, walking),
            _ => {}
        }
    }

    fn protect(&mut self, stmt: &Stmt, name: &str, whole: bool, walking: &[Loop]) {
        let append = matches!(stmt.kind, StmtKind::Append { .. });
        let msg = match walking.iter().rev().find(|l| l.name() == name) {
            Some(_) if !whole => return,
            Some(Loop::Item(_, list)) if append => {
                format!("{name} stands for an item of {list}, so nothing can be appended to it")
            }
            Some(Loop::Item(_, list)) => format!(
                "{name} stands for an item of {list} and cannot be set. To change the item, loop with for each i from 1 to count of {list} and set item i of {list}. To work on a copy, set m to {name} first"
            ),
            Some(Loop::Count(_)) => format!("{name} counts the loop and cannot be changed"),
            None if self.inputs.iter().any(|(n, _)| n == name) => format!(
                "{name} is an input, so it cannot be changed. To work on a copy, set mine to {name} first"
            ),
            None => return,
        };
        let d = Diagnostic::error(self.ctx.src, stmt.line, msg);
        self.ctx.diags.push(d);
    }
}

#[cfg(test)]
mod tests {
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn appending_to_a_loop_variable_is_worded_for_append() {
        let all =
            diags("feature start\n  set ns to list of 1\n  for each n in ns\n    append 2 to n\n");
        let msg =
            "line 4: append 2 to n. n stands for an item of ns, so nothing can be appended to it.";
        assert!(all.contains(&msg.to_string()), "{all:?}");
        assert!(!all.iter().any(|d| d.contains("cannot be set")), "{all:?}");
    }

    #[test]
    fn appending_to_a_loop_variable_is_one_diagnostic() {
        assert_eq!(
            diags("feature start\n  for each i from 1 to 3\n    append 1 to i\n"),
            ["line 3: append 1 to i. i counts the loop and cannot be changed."]
        );
        let item =
            diags("feature start\n  set ns to list of 1\n  for each n in ns\n    append 2 to n\n");
        assert_eq!(item.len(), 1, "{item:?}");
    }

    #[test]
    fn loop_and_input_mutation() {
        assert_eq!(
            diags("feature start\n  set ns to list of 1\n  for each n in ns\n    add 1 to n\n"),
            [
                "line 4: add 1 to n. n stands for an item of ns and cannot be set. To change the item, loop with for each i from 1 to count of ns and set item i of ns. To work on a copy, set m to n first."
            ]
        );
        assert_eq!(
            diags("feature start\n  for each i from 1 to 3\n    set i to 2\n"),
            ["line 3: set i to 2. i counts the loop and cannot be changed."]
        );
        assert!(diags(&format!("{HEAD}feature start\n  load es\n  for each e in es\n    set amount of e to 0\n  save es\n")).is_empty());
        assert_eq!(
            diags(
                "feature start\n  run f with (list of 1)\nfeature f\n  needs ns as list of number\n  append 2 to ns\n"
            ),
            [
                "line 5: append 2 to ns. ns is an input, so it cannot be changed. To work on a copy, set mine to ns first."
            ]
        );
    }
}
