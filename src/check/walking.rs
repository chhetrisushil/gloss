use std::collections::{BTreeMap, BTreeSet};

use super::Ctx;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::walk::{self, blocks};
use crate::ast::{Expr, ExprKind, Line};
use crate::diag::Diagnostic;

/// For each feature: the tables it changes itself, and the features it calls.
/// Features have separate scopes, so only tables are shared between them.
struct Graph<'a> {
    tables: BTreeSet<&'a str>,
    changes: BTreeMap<&'a str, BTreeSet<&'a str>>,
    calls: BTreeMap<&'a str, Vec<&'a str>>,
}

/// The list a statement appends to, sets, clears or loads.
fn restructures(kind: &StmtKind) -> Option<&str> {
    match kind {
        StmtKind::Append { list: name, .. }
        | StmtKind::Clear(name)
        | StmtKind::Load(name)
        | StmtKind::Set {
            target: Target::Var(name),
            ..
        } => Some(name),
        _ => None,
    }
}

/// Every feature a statement runs or an expression of it calls, not looking inside blocks.
pub(super) fn called<'a>(kind: &'a StmtKind, out: &mut Vec<&'a str>) {
    if let StmtKind::Run { feature, .. } = kind {
        out.push(feature);
    }
    walk::for_each_expr_of(kind, &mut |e| {
        if let ExprKind::Call { feature, .. } = &e.kind {
            out.push(feature);
        }
    });
}

impl<'a> Graph<'a> {
    fn collect(&mut self, name: &'a str, body: &'a [Stmt]) {
        for stmt in body {
            let own = self.changes.entry(name).or_default();
            own.extend(restructures(&stmt.kind).filter(|n| self.tables.contains(n)));
            called(&stmt.kind, self.calls.entry(name).or_default());
            for b in blocks(&stmt.kind) {
                self.collect(name, b);
            }
        }
    }

    /// The features from `from` down to one that changes `list`, if any.
    fn path(
        &self,
        from: &'a str,
        list: &str,
        seen: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<&'a str>> {
        if !seen.insert(from) {
            return None;
        }
        if self.changes.get(from).is_some_and(|s| s.contains(list)) {
            return Some(vec![from]);
        }
        let next = self.calls.get(from)?;
        next.iter().find_map(|n| {
            let mut path = self.path(n, list, seen)?;
            path.insert(0, from);
            Some(path)
        })
    }
}

/// Rejects changes to a list inside a loop that walks it, directly or through calls.
pub fn check_walking(ctx: &mut Ctx) {
    let program = ctx.program;
    let mut graph = Graph {
        tables: program.tables().map(|t| t.name.as_str()).collect(),
        changes: BTreeMap::new(),
        calls: BTreeMap::new(),
    };
    for f in program.features() {
        graph.collect(&f.name, &f.body);
    }
    let bodies = program.features().map(|f| &f.body);
    for body in bodies.chain(program.tests().map(|t| &t.body)) {
        walk(ctx, &graph, body, &mut Vec::new());
    }
}

/// The name a loop walks, looking through parentheses. Records cannot hold lists, so a name
/// (a variable or a table) is the only way a loop can walk something that can be changed.
fn list_name(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Name(name) => Some(name),
        ExprKind::Paren(inner) => list_name(inner),
        _ => None,
    }
}

fn walk<'a>(ctx: &mut Ctx, graph: &Graph, body: &'a [Stmt], walked: &mut Vec<(&'a str, Line)>) {
    for stmt in body {
        let mut runs = Vec::new();
        called(&stmt.kind, &mut runs);
        for (list, line) in walked.iter().rev() {
            let cause = if restructures(&stmt.kind) == Some(list) {
                Some(String::new())
            } else {
                runs.iter().find_map(|f| {
                    let path = graph.path(f, list, &mut BTreeSet::new())?;
                    let through = &path[1..];
                    let via = if through.is_empty() {
                        String::new()
                    } else {
                        format!(" (through {})", through.join(", "))
                    };
                    Some(format!(": {} changes it{via}", path[0]))
                })
            };
            if let Some(cause) = cause {
                let msg =
                    format!("{list} cannot change while the loop on line {line} walks it{cause}");
                ctx.diags.push(Diagnostic::error(ctx.src, stmt.line, msg));
                break;
            }
        }
        for b in blocks(&stmt.kind) {
            let before = walked.len();
            if let StmtKind::ForEachIn { list, .. } = &stmt.kind {
                if let Some(name) = list_name(list) {
                    walked.push((name, stmt.line));
                }
            }
            walk(ctx, graph, b, walked);
            walked.truncate(before);
        }
    }
}

#[cfg(test)]
mod tests {
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn walking() {
        let p = format!(
            "{HEAD}feature start\n  load es\n  for each e in es\n    run add_one\n  save es\nfeature add_one\n  run put\nfeature put\n  append (E with amount 1) to es\n"
        );
        assert_eq!(
            diags(&p),
            [
                "line 7: run add_one. es cannot change while the loop on line 6 walks it: add_one changes it (through put)."
            ]
        );
    }
    #[test]
    fn walking_a_local_list() {
        let p = "feature start\n  set ns to list of 1\n  for each n in ns\n    append 2 to ns\n    show n\n";
        assert_eq!(
            diags(p),
            ["line 4: append 2 to ns. ns cannot change while the loop on line 3 walks it."]
        );
    }
    #[test]
    fn walking_through_parentheses() {
        let p = "feature start\n  set ns to list of 1\n  for each n in (ns)\n    append 2 to ns\n    show n\n";
        assert_eq!(
            diags(p),
            ["line 4: append 2 to ns. ns cannot change while the loop on line 3 walks it."]
        );
    }
    #[test]
    fn a_callee_changing_its_own_local_list_is_fine() {
        let p = "feature start\n  set items to list of 1\n  for each n in items\n    run helper\n    show n\nfeature helper\n  set items to list of 2\n  show items\n";
        assert!(diags(p).is_empty());
    }
}
