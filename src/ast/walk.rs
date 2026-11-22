//! The one way to visit the statements and expressions of a program: what a statement holds
//! directly, what an expression holds, and visitors built on them.

use super::stmt::{Stmt, StmtKind, Target};
use super::{Expr, ExprKind};

/// The expressions a statement holds directly, and the blocks inside it. The position inside an
/// item target is one of the expressions.
pub fn parts<'a>(kind: &'a StmtKind, exprs: &mut Vec<&'a Expr>, nested: &mut Vec<&'a [Stmt]>) {
    match kind {
        StmtKind::Set { target, value }
        | StmtKind::Add { value, target }
        | StmtKind::Subtract { value, target }
        | StmtKind::Multiply { target, by: value }
        | StmtKind::Divide { target, by: value } => {
            target_exprs(target, exprs);
            exprs.push(value);
        }
        StmtKind::Append { value: x, .. } | StmtKind::Ask { question: x, .. } => exprs.push(x),
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
        StmtKind::Note(_)
        | StmtKind::Skip
        | StmtKind::Stop
        | StmtKind::Load(_)
        | StmtKind::Save(_)
        | StmtKind::Clear(_) => {}
    }
}

pub fn children(e: &Expr) -> Vec<&Expr> {
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
        ExprKind::Number(_)
        | ExprKind::Text(_)
        | ExprKind::Truth(_)
        | ExprKind::Name(_)
        | ExprKind::Shown
        | ExprKind::EmptyList(_) => Vec::new(),
    }
}

/// Like `parts`, for editing; the position inside an item target is one of the expressions.
pub fn parts_mut<'a>(
    kind: &'a mut StmtKind,
    exprs: &mut Vec<&'a mut Expr>,
    nested: &mut Vec<&'a mut [Stmt]>,
) {
    match kind {
        StmtKind::Set { target, value }
        | StmtKind::Add { value, target }
        | StmtKind::Subtract { value, target }
        | StmtKind::Multiply { target, by: value }
        | StmtKind::Divide { target, by: value } => {
            target_exprs_mut(target, exprs);
            exprs.push(value);
        }
        StmtKind::Append { value: x, .. }
        | StmtKind::Ask { question: x, .. }
        | StmtKind::Give(x)
        | StmtKind::Expect(x) => exprs.push(x),
        StmtKind::Show(xs) | StmtKind::Input(xs) | StmtKind::Run { inputs: xs, .. } => {
            exprs.extend(xs);
        }
        StmtKind::If {
            branches,
            otherwise,
        } => {
            for (cond, b) in branches {
                exprs.push(cond);
                nested.push(b);
            }
            nested.extend(otherwise.as_deref_mut());
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
        StmtKind::Note(_)
        | StmtKind::Skip
        | StmtKind::Stop
        | StmtKind::Load(_)
        | StmtKind::Save(_)
        | StmtKind::Clear(_) => {}
    }
}

fn target_exprs<'a>(target: &'a Target, out: &mut Vec<&'a Expr>) {
    match target {
        Target::Var(_) => {}
        Target::Field { of, .. } => target_exprs(of, out),
        Target::Item { position, of } => {
            out.push(position);
            target_exprs(of, out);
        }
    }
}

fn target_exprs_mut<'a>(target: &'a mut Target, out: &mut Vec<&'a mut Expr>) {
    match target {
        Target::Var(_) => {}
        Target::Field { of, .. } => target_exprs_mut(of, out),
        Target::Item { position, of } => {
            out.push(position);
            target_exprs_mut(of, out);
        }
    }
}

/// The blocks directly inside a statement.
pub fn blocks(kind: &StmtKind) -> Vec<&[Stmt]> {
    let (mut exprs, mut nested) = (Vec::new(), Vec::new());
    parts(kind, &mut exprs, &mut nested);
    nested
}

/// Calls `f` on every statement of `body`, each one before the statements inside it.
pub fn for_each_stmt<'a>(body: &'a [Stmt], f: &mut impl FnMut(&'a Stmt)) {
    for stmt in body {
        f(stmt);
        for block in blocks(&stmt.kind) {
            for_each_stmt(block, f);
        }
    }
}

/// Calls `f` on `e` and every expression inside it, each one before its parts.
pub fn for_each_expr<'a>(e: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    f(e);
    for child in children(e) {
        for_each_expr(child, f);
    }
}

/// Calls `f` on every expression a statement holds directly, and on those inside them.
pub fn for_each_expr_of<'a>(kind: &'a StmtKind, f: &mut impl FnMut(&'a Expr)) {
    let mut exprs = Vec::new();
    parts(kind, &mut exprs, &mut Vec::new());
    for e in exprs {
        for_each_expr(e, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_statements_and_expressions_in_source_order() {
        let text = "feature start\n  set a to 1 + b\n  if a is 2\n    show a, (c with 1)\n";
        let program = crate::parse_source(&crate::source::Source::new(text)).unwrap();
        let (mut stmts, mut names) = (0, Vec::new());
        for_each_stmt(&program.features().next().unwrap().body, &mut |s| {
            stmts += 1;
            for_each_expr_of(&s.kind, &mut |e| {
                if let ExprKind::Name(n) = &e.kind {
                    names.push(n.as_str());
                }
            });
        });
        assert_eq!((stmts, names), (3, ["b", "a", "a"].to_vec()));
    }
}
