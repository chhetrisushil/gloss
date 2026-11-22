//! The one way to look inside statements and expressions: what a statement holds directly
//! and what an expression holds. Every pass that walks a program starts from these.

use super::stmt::{Stmt, StmtKind};
use super::{Expr, ExprKind};

/// The expressions a statement holds directly, and the blocks inside it.
pub fn parts<'a>(kind: &'a StmtKind, exprs: &mut Vec<&'a Expr>, nested: &mut Vec<&'a [Stmt]>) {
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
        _ => Vec::new(),
    }
}

/// The blocks directly inside a statement.
pub fn blocks(kind: &StmtKind) -> Vec<&[Stmt]> {
    let (mut exprs, mut nested) = (Vec::new(), Vec::new());
    parts(kind, &mut exprs, &mut nested);
    nested
}
