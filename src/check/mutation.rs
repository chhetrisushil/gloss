use super::{Ctx, Ty};
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{Expr, ExprKind, print};
use crate::diag::Diagnostic;

/// What a loop variable stands for: an item of a list, or a count.
/// `list` without the parentheses around a plain name, so a message names `xs`, not `(xs)`.
fn bare(list: &Expr) -> &Expr {
    match &list.kind {
        ExprKind::Paren(inner) if matches!(bare(inner).kind, ExprKind::Name(_)) => bare(inner),
        _ => list,
    }
}

enum Loop<'a> {
    /// The loop variable, the printed list, and whether that list comes from an input.
    Item(&'a str, String, bool),
    Count(&'a str),
}

impl Loop<'_> {
    fn name(&self) -> &str {
        match self {
            Loop::Item(name, ..) | Loop::Count(name) => name,
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
                let walks_input = self.walks_input(list, walking);
                walking.push(Loop::Item(var, print::expr(bare(list)), walks_input));
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

    /// Whether the list a loop walks comes from an input, directly or through another loop.
    fn walks_input(&self, list: &Expr, walking: &[Loop]) -> bool {
        let root = match &list.kind {
            ExprKind::Name(name) => name,
            ExprKind::FieldOf { of, .. } | ExprKind::ItemOf { of, .. } | ExprKind::Paren(of) => {
                return self.walks_input(of, walking);
            }
            _ => return false,
        };
        match walking.iter().rev().find(|l| l.name() == root) {
            Some(Loop::Item(.., walks)) => *walks,
            Some(Loop::Count(_)) => false,
            None => self.inputs.iter().any(|(n, _)| n == root),
        }
    }

    fn protect(&mut self, stmt: &Stmt, name: &str, whole: bool, walking: &[Loop]) {
        let append = matches!(stmt.kind, StmtKind::Append { .. });
        let msg = match walking.iter().rev().find(|l| l.name() == name) {
            Some(Loop::Item(_, list, true)) if !whole => format!(
                "{name} stands for an item of {list}, which is an input, so it cannot be changed"
            ),
            Some(_) if !whole => return,
            Some(Loop::Item(_, list, _)) if append => {
                format!("{name} stands for an item of {list}, so nothing can be appended to it")
            }
            Some(Loop::Item(_, list, _)) => format!(
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
    fn changing_an_input_through_a_loop_variable_is_reported() {
        let head = "record E\n  amount as number\ntable es of E in \"e.csv\"\nfeature start\n  run zero with es\n";
        let zero = |body: &str| {
            diags(&format!(
                "{head}feature zero\n  needs xs as list of E\n{body}"
            ))
        };
        let msg = "which is an input, so it cannot be changed.";
        let direct = zero("  for each x in xs\n    set amount of x to 0\n");
        assert_eq!(direct.len(), 1, "{direct:?}");
        assert!(
            direct[0].ends_with(&format!("x stands for an item of xs, {msg}")),
            "{direct:?}"
        );
        let nested =
            zero("  for each x in xs\n    for each y in (list of x)\n      set amount of y to 0\n");
        assert!(nested.iter().all(|d| !d.contains("an input")), "{nested:?}");
        let own = diags(&format!(
            "{HEAD}feature start\n  set mine to empty list of E\n  for each e in mine\n    set amount of e to 0\n"
        ));
        assert!(own.is_empty(), "{own:?}");
    }

    #[test]
    fn changing_an_input_through_a_parenthesised_loop_list_is_reported() {
        let all = diags(
            "record P\n  age as number\nfeature start\n  set ps to list of (P with age 1)\n  run bump with ps\nfeature bump\n  needs xs as list of P\n  for each p in (xs)\n    set age of p to 99\n",
        );
        assert_eq!(all.len(), 1, "{all:?}");
        assert!(
            all[0].ends_with(
                "p stands for an item of xs, which is an input, so it cannot be changed."
            ),
            "{all:?}"
        );
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
