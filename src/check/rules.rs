use super::Ctx;
use crate::ast::print;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{ArithOp, Expr, ExprKind};
use crate::diag::Diagnostic;

/// The verb form of a `set` whose value is its own target combined with one other value.
pub fn set_rewrite(stmt: &StmtKind) -> Option<StmtKind> {
    let StmtKind::Set { target, value } = stmt else {
        return None;
    };
    let ExprKind::Binary { op, left, right } = &value.kind else {
        return None;
    };
    let (left, right) = ((**left).clone(), (**right).clone());
    let target = target.clone();
    let (own_left, own_right) = (is_target(&target, &left), is_target(&target, &right));
    match op {
        ArithOp::Add if own_left => Some(StmtKind::Add {
            value: right,
            target,
        }),
        ArithOp::Add if own_right => Some(StmtKind::Add {
            value: left,
            target,
        }),
        ArithOp::Sub if own_left => Some(StmtKind::Subtract {
            value: right,
            target,
        }),
        ArithOp::Mul if own_left => Some(StmtKind::Multiply { target, by: right }),
        ArithOp::Mul if own_right => Some(StmtKind::Multiply { target, by: left }),
        ArithOp::Div if own_left => Some(StmtKind::Divide { target, by: right }),
        _ => None,
    }
}

/// True when the expression names the target, ignoring ids and lines.
fn is_target(t: &Target, e: &Expr) -> bool {
    target_text(t) == print::expr(e)
}

pub fn target_text(t: &Target) -> String {
    match t {
        Target::Var(name) => name.clone(),
        Target::Field { field, of } => format!("{field} of {}", target_text(of)),
        Target::Item { position, of } => {
            format!("item {} of {}", print::expr(position), target_text(of))
        }
    }
}

/// The verb form as the user would write it, its verb, and the symbol it replaces.
fn form_text(kind: &StmtKind) -> Option<(String, &'static str, &'static str)> {
    Some(match kind {
        StmtKind::Add { value, target } => (
            format!("add {} to {}", print::expr(value), target_text(target)),
            "add",
            "+",
        ),
        StmtKind::Subtract { value, target } => (
            format!(
                "subtract {} from {}",
                print::expr(value),
                target_text(target)
            ),
            "subtract",
            "-",
        ),
        StmtKind::Multiply { target, by } => (
            format!("multiply {} by {}", target_text(target), print::expr(by)),
            "multiply",
            "*",
        ),
        StmtKind::Divide { target, by } => (
            format!("divide {} by {}", target_text(target), print::expr(by)),
            "divide",
            "/",
        ),
        _ => return None,
    })
}

/// Rejects arithmetic `set`s that have a verb form.
pub fn check_rules(ctx: &mut Ctx, body: &[Stmt]) {
    for stmt in body {
        if let Some((form, verb, sign)) = set_rewrite(&stmt.kind).as_ref().and_then(form_text) {
            let msg = format!("set with {sign} on its own target is written with {verb}");
            ctx.diags
                .push(Diagnostic::error(ctx.src, stmt.line, msg).suggest(form));
        }
        nested(&stmt.kind)
            .into_iter()
            .for_each(|b| check_rules(ctx, b));
    }
}

/// The blocks inside a statement.
fn nested(kind: &StmtKind) -> Vec<&[Stmt]> {
    match kind {
        StmtKind::If {
            branches,
            otherwise,
        } => branches
            .iter()
            .map(|(_, b)| b.as_slice())
            .chain(otherwise.as_deref())
            .collect(),
        StmtKind::ForEachIn { body, .. }
        | StmtKind::ForEachRange { body, .. }
        | StmtKind::RepeatWhile { body, .. } => vec![body],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn rule_two_sets() {
        assert_eq!(
            diags("feature start\n  set t to 1\n  set t to t + 2\n  show t\n"),
            [
                "line 3: set t to t + 2. set with + on its own target is written with add. Did you mean add 2 to t?"
            ]
        );
        assert_eq!(
            diags("feature start\n  set t to 1\n  set t to 3 * t\n  show t\n"),
            [
                "line 3: set t to 3 * t. set with * on its own target is written with multiply. Did you mean multiply t by 3?"
            ]
        );
        assert!(
            diags(
                "feature start\n  set t to 1\n  set t to 3 - t\n  set t to t * 2 + 1\n  show t\n"
            )
            .is_empty()
        );
    }
    #[test]
    fn other_set_forms() {
        let rec = "record E\n  amount as number\nfeature start\n  set e to E with amount 1\n";
        assert_eq!(
            diags(&format!(
                "{rec}  set amount of e to amount of e + 1\n  show amount of e\n"
            )),
            [
                "line 5: set amount of e to amount of e + 1. set with + on its own target is written with add. Did you mean add 1 to amount of e?"
            ]
        );
        assert_eq!(
            diags("feature start\n  set t to 1\n  set t to 2 + t\n  show t\n"),
            [
                "line 3: set t to 2 + t. set with + on its own target is written with add. Did you mean add 2 to t?"
            ]
        );
        assert_eq!(
            diags("feature start\n  set t to 1\n  set t to t - 2\n  show t\n"),
            [
                "line 3: set t to t - 2. set with - on its own target is written with subtract. Did you mean subtract 2 from t?"
            ]
        );
        assert_eq!(
            diags("feature start\n  set t to 1\n  set t to t / 2\n  show t\n"),
            [
                "line 3: set t to t / 2. set with / on its own target is written with divide. Did you mean divide t by 2?"
            ]
        );
        assert!(
            diags("feature start\n  set t to 1\n  set t to (t + 2)\n  set t to 2 - t\n  show t\n")
                .is_empty()
        );
    }
}
