use super::{Ctx, Ty};
use crate::ast::print;
use crate::ast::sexpr::sexpr;
use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::{ArithOp, CompareOp, Expr, ExprKind};
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

/// The form of `X is yes`, `X is not no` and the like that uses X directly.
pub fn truth_rewrite(e: &Expr) -> Option<ExprKind> {
    let ExprKind::Compare { op, left, right } = &e.kind else {
        return None;
    };
    if !matches!(op, CompareOp::Is | CompareOp::IsNot) {
        return None;
    }
    let (x, yes) = match (&left.kind, &right.kind) {
        (_, ExprKind::Truth(b)) => (left, *b),
        (ExprKind::Truth(b), _) => (right, *b),
        _ => return None,
    };
    if *op == CompareOp::Is && yes || *op == CompareOp::IsNot && !yes {
        Some(x.kind.clone())
    } else {
        Some(ExprKind::Not(x.clone()))
    }
}

/// True when the expression names the target, ignoring ids and lines.
fn is_target(t: &Target, e: &Expr) -> bool {
    let bare = e.without_parens();
    match (t, &bare.kind) {
        (Target::Var(name), ExprKind::Name(other)) => name == other,
        (Target::Field { field, of }, ExprKind::FieldOf { field: f, of: o }) => {
            field == f && is_target(of, o)
        }
        (Target::Item { position, of }, ExprKind::ItemOf { position: p, of: o }) => {
            sexpr(&position.without_parens()) == sexpr(p) && is_target(of, o)
        }
        _ => false,
    }
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

/// Rejects arithmetic `set`s and truth comparisons that have a shorter form.
pub fn check_rules(ctx: &mut Ctx, types: &[Option<Ty>], body: &[Stmt]) {
    for stmt in body {
        if let Some((form, verb, sign)) = set_rewrite(&stmt.kind).as_ref().and_then(form_text) {
            let msg = format!("set with {sign} on its own target is written with {verb}");
            ctx.diags
                .push(Diagnostic::error(ctx.src, stmt.line, msg).suggest(form));
        }
        let mut exprs = Vec::new();
        let mut nested: Vec<&[Stmt]> = Vec::new();
        parts(&stmt.kind, &mut exprs, &mut nested);
        exprs.into_iter().for_each(|e| check_truth(ctx, types, e));
        nested.into_iter().for_each(|b| check_rules(ctx, types, b));
    }
}

fn check_truth(ctx: &mut Ctx, types: &[Option<Ty>], e: &Expr) {
    if let Some(kind) = truth_rewrite(e).filter(|_| compares_a_truth(types, e)) {
        let better = print::expr(&Expr {
            id: e.id,
            line: e.line,
            kind,
        });
        let msg = format!(
            "A truth is used directly: write {better} instead of {}",
            print::expr(e)
        );
        ctx.diags.push(Diagnostic::error(ctx.src, e.line, msg));
    }
    children(e)
        .into_iter()
        .for_each(|c| check_truth(ctx, types, c));
}

/// True when the non-literal side of the comparison is a truth; other types
/// already have their own error.
fn compares_a_truth(types: &[Option<Ty>], e: &Expr) -> bool {
    let ExprKind::Compare { left, right, .. } = &e.kind else {
        return false;
    };
    [left, right].iter().any(|x| {
        let known = types.get(x.id.0 as usize).cloned().flatten();
        !matches!(x.kind, ExprKind::Truth(_)) && known == Some(Ty::Truth)
    })
}

/// The expressions a statement holds directly, and the blocks inside it.
pub(super) fn parts<'a>(
    kind: &'a StmtKind,
    exprs: &mut Vec<&'a Expr>,
    nested: &mut Vec<&'a [Stmt]>,
) {
    match kind {
        StmtKind::Set { value, .. } => exprs.push(value),
        StmtKind::Add { value, .. } | StmtKind::Subtract { value, .. } => exprs.push(value),
        StmtKind::Multiply { by, .. } | StmtKind::Divide { by, .. } => exprs.push(by),
        StmtKind::Append { value, .. } => exprs.push(value),
        StmtKind::Ask { question, .. } => exprs.push(question),
        StmtKind::Show(xs) | StmtKind::Input(xs) | StmtKind::Run { inputs: xs, .. } => {
            exprs.extend(xs);
        }
        StmtKind::Give(e) | StmtKind::Expect(e) => exprs.push(e),
        StmtKind::If {
            branches,
            otherwise,
        } => {
            for (cond, b) in branches {
                exprs.push(cond);
                nested.push(b);
            }
            nested.extend(otherwise.as_deref());
        }
        StmtKind::ForEachIn { list, body, .. } => {
            exprs.push(list);
            nested.push(body);
        }
        StmtKind::ForEachRange { from, to, body, .. } => {
            exprs.extend([from, to]);
            nested.push(body);
        }
        StmtKind::RepeatWhile { cond, body } => {
            exprs.push(cond);
            nested.push(body);
        }
        _ => {}
    }
}

pub(super) fn children(e: &Expr) -> Vec<&Expr> {
    match &e.kind {
        ExprKind::Paren(x)
        | ExprKind::FieldOf { of: x, .. }
        | ExprKind::Fact { of: x, .. }
        | ExprKind::Negate(x)
        | ExprKind::Convert { value: x, .. }
        | ExprKind::IsNumber { value: x, .. }
        | ExprKind::Not(x) => vec![x],
        ExprKind::ItemOf { position: a, of: b }
        | ExprKind::Remainder { of: a, by: b }
        | ExprKind::Binary {
            left: a, right: b, ..
        }
        | ExprKind::Compare {
            left: a, right: b, ..
        } => vec![a, b],
        ExprKind::Call { inputs: xs, .. }
        | ExprKind::BuildList(xs)
        | ExprKind::And(xs)
        | ExprKind::Or(xs) => xs.iter().collect(),
        ExprKind::BuildRecord { fields, .. } => fields.iter().map(|(_, x)| x).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }

    #[test]
    fn rule_two() {
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
        assert_eq!(
            diags("feature start\n  set d to yes\n  if d is yes\n    show 1\n"),
            ["line 3: if d is yes. A truth is used directly: write d instead of d is yes."]
        );
        assert_eq!(
            diags("feature start\n  set d to yes\n  if d is not yes\n    show 1\n"),
            [
                "line 3: if d is not yes. A truth is used directly: write not d instead of d is not yes."
            ]
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
    #[test]
    fn other_truth_forms() {
        let d = "feature start\n  set d to yes\n";
        assert_eq!(
            diags(&format!("{d}  if yes is d\n    show 1\n")),
            ["line 3: if yes is d. A truth is used directly: write d instead of yes is d."]
        );
        assert_eq!(
            diags(&format!("{d}  if d is no\n    show 1\n")),
            ["line 3: if d is no. A truth is used directly: write not d instead of d is no."]
        );
        assert_eq!(
            diags(&format!("{d}  if d is not no\n    show 1\n")),
            ["line 3: if d is not no. A truth is used directly: write d instead of d is not no."]
        );
    }

    #[test]
    fn a_non_truth_gets_only_its_type_error() {
        let got = diags("feature start\n  set n to 1\n  if n is yes\n    show 1\n");
        assert_eq!(got.len(), 1);
        assert!(!got[0].contains("A truth is used directly"));
    }
}
