use std::collections::{BTreeMap, BTreeSet};

use super::Ctx;
use crate::ast::Line;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::walk::{blocks, for_each_stmt};
use crate::diag::Diagnostic;

/// Rejects a table that is saved but never loaded or cleared, and warns when a
/// `load` replaces rows appended earlier in the same block.
pub fn check_saving(ctx: &mut Ctx) {
    let program = ctx.program;
    let features: Vec<&[Stmt]> = program.features().map(|f| f.body.as_slice()).collect();
    // Only the executable's own features load: a test uses pretend files (M4), so a `load`
    // there would leave the built program stopping at its first `save` (failure 8).
    let mut loaded = BTreeSet::new();
    features.iter().for_each(|b| loaded_in(b, &mut loaded));
    let mut reported = BTreeSet::new();
    let tests = program.tests().map(|t| t.body.as_slice());
    for body in features.iter().copied().chain(tests) {
        unloaded_saves(ctx, body, &loaded, &mut reported);
        load_after_append(ctx, body, &mut BTreeMap::new());
    }
}

fn loaded_in<'a>(body: &'a [Stmt], out: &mut BTreeSet<&'a str>) {
    for_each_stmt(body, &mut |stmt| {
        if let StmtKind::Load(t) | StmtKind::Clear(t) = &stmt.kind {
            out.insert(t);
        }
    });
}

fn unloaded_saves<'a>(
    ctx: &mut Ctx,
    body: &'a [Stmt],
    loaded: &BTreeSet<&str>,
    reported: &mut BTreeSet<&'a str>,
) {
    for stmt in body {
        if let StmtKind::Save(t) = &stmt.kind {
            let known = ctx.program.tables().any(|d| &d.name == t);
            if known && !loaded.contains(t.as_str()) && reported.insert(t) {
                let msg = format!(
                    "{t} is saved here but nothing in this program loads it. Add load {t} before the first change, or clear {t} if you mean to start the file again"
                );
                ctx.diags.push(Diagnostic::error(ctx.src, stmt.line, msg));
            }
        }
        for b in blocks(&stmt.kind) {
            unloaded_saves(ctx, b, loaded, reported);
        }
    }
}

/// `appended` is shared with the blocks inside `body`, so an append anywhere earlier in the
/// feature counts, including one inside an if or a loop.
fn load_after_append<'a>(ctx: &mut Ctx, body: &'a [Stmt], appended: &mut BTreeMap<&'a str, Line>) {
    for stmt in body {
        match &stmt.kind {
            StmtKind::Append { list, .. } => {
                appended.entry(list).or_insert(stmt.line);
            }
            StmtKind::Load(t) => {
                let known = ctx.program.tables().any(|d| &d.name == t);
                if let Some(line) = appended.remove(t.as_str()).filter(|_| known) {
                    let msg = format!("load {t} replaces the rows appended on line {line}");
                    ctx.diags.push(Diagnostic::warning(ctx.src, stmt.line, msg));
                }
            }
            StmtKind::Clear(t) => {
                appended.remove(t.as_str());
            }
            _ => {}
        }
        blocks(&stmt.kind)
            .into_iter()
            .for_each(|b| load_after_append(ctx, b, appended));
    }
}

#[cfg(test)]
mod tests {
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn saving() {
        assert_eq!(
            diags(&format!("{HEAD}feature start\n  save es\n")),
            [
                "line 5: save es. es is saved here but nothing in this program loads it. Add load es before the first change, or clear es if you mean to start the file again."
            ]
        );
        assert_eq!(
            diags(&format!(
                "{HEAD}table fs of E in \"f.csv\"\nfeature start\n  load fs\n  save es\n"
            )),
            [
                "line 7: save es. es is saved here but nothing in this program loads it. Add load es before the first change, or clear es if you mean to start the file again."
            ]
        );
        assert_eq!(
            diags(&format!(
                "{HEAD}feature start\n  append (E with amount 1) to es\n  load es\n  save es\n"
            )),
            ["line 6: warning: load es. load es replaces the rows appended on line 5."]
        );
        let warns = |body: &str| diags(&format!("{HEAD}feature start\n{body}  save es\n"));
        let msg = "line 7: warning: load es. load es replaces the rows appended on line 6.";
        let inside_if = "  if 1 is 1\n    append (E with amount 1) to es\n  load es\n";
        assert_eq!(
            warns(inside_if),
            ["line 7: warning: load es. load es replaces the rows appended on line 6."]
        );
        let in_loop = "  for each i from 1 to 2\n    append (E with amount i) to es\n  load es\n";
        assert_eq!(warns(in_loop), [msg]);
        let load_inside = "  append (E with amount 1) to es\n  if 1 is 1\n    load es\n";
        assert_eq!(warns(load_inside).len(), 1);
        assert!(
            diags(&format!(
                "{HEAD}feature start\n  clear es\n  append (E with amount 1) to es\n  save es\n"
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_load_in_a_test_does_not_count_for_a_save_in_a_feature() {
        let only_in_test = format!("{HEAD}feature start\n  save es\ntest \"t\"\n  load es\n");
        assert_eq!(
            diags(&only_in_test),
            [
                "line 5: save es. es is saved here but nothing in this program loads it. Add load es before the first change, or clear es if you mean to start the file again."
            ]
        );
        let in_feature =
            format!("{HEAD}feature start\n  load es\n  save es\ntest \"t\"\n  load es\n");
        assert!(diags(&in_feature).is_empty());
    }

    #[test]
    fn loading_a_list_only_gets_the_load_error() {
        let p = "feature start\n  set mine to list of 1\n  append 2 to mine\n  load mine\n  show mine\n";
        assert_eq!(
            diags(p),
            ["line 4: load mine. load works on tables; mine is a list of number."]
        );
    }
}
