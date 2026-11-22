use crate::ast::program::{Item, Program};
use crate::ast::stmt::{Stmt, StmtKind as S, Target};
use crate::ast::{Expr, ExprKind as K};
use crate::check::rules::{set_rewrite, truth_rewrite};

/// The program with the spec's rewrites applied everywhere.
pub fn rewrite(program: &Program) -> Program {
    let mut out = program.clone();
    for item in &mut out.items {
        match item {
            Item::Feature(f) => body(&mut f.body),
            Item::Test(t) => body(&mut t.body),
            _ => {}
        }
    }
    out
}

fn body(stmts: &mut [Stmt]) {
    for s in stmts {
        stmt(&mut s.kind);
        if let Some(verb) = set_rewrite(&s.kind) {
            s.kind = verb;
        }
    }
}

fn stmt(kind: &mut S) {
    match kind {
        S::Set {
            target: t,
            value: x,
        }
        | S::Add {
            value: x,
            target: t,
        }
        | S::Subtract {
            value: x,
            target: t,
        }
        | S::Multiply { target: t, by: x }
        | S::Divide { target: t, by: x } => {
            target(t);
            expr(x);
        }
        S::Append { value: x, .. }
        | S::Ask { question: x, .. }
        | S::Give(x)
        | S::Expect(x)
        | S::RepeatWhile { cond: x, .. } => expr(x),
        S::Show(xs) | S::Input(xs) | S::Run { inputs: xs, .. } => xs.iter_mut().for_each(expr),
        S::ForEachIn { list, .. } => expr(list),
        S::ForEachRange { from, to, .. } => {
            expr(from);
            expr(to);
        }
        S::If { branches, .. } => branches.iter_mut().for_each(|(c, _)| expr(c)),
        _ => {}
    }
    match kind {
        S::If {
            branches,
            otherwise,
        } => {
            branches.iter_mut().for_each(|(_, b)| body(b));
            otherwise.iter_mut().for_each(|b| body(b));
        }
        S::ForEachIn { body: b, .. }
        | S::ForEachRange { body: b, .. }
        | S::RepeatWhile { body: b, .. } => body(b),
        _ => {}
    }
}

fn target(t: &mut Target) {
    match t {
        Target::Var(_) => {}
        Target::Field { of, .. } => target(of),
        Target::Item { position, of } => {
            expr(position);
            target(of);
        }
    }
}

fn expr(e: &mut Expr) {
    match &mut e.kind {
        K::Paren(x)
        | K::FieldOf { of: x, .. }
        | K::Fact { of: x, .. }
        | K::Negate(x)
        | K::Convert { value: x, .. }
        | K::IsNumber { value: x, .. }
        | K::Not(x) => expr(x),
        K::ItemOf { position: a, of: b }
        | K::Remainder { of: a, by: b }
        | K::Binary {
            left: a, right: b, ..
        }
        | K::Compare {
            left: a, right: b, ..
        } => {
            expr(a);
            expr(b);
        }
        K::Call { inputs: xs, .. } | K::BuildList(xs) | K::And(xs) | K::Or(xs) => {
            xs.iter_mut().for_each(expr)
        }
        K::BuildRecord { fields, .. } => fields.iter_mut().for_each(|(_, x)| expr(x)),
        _ => {}
    }
    if let Some(kind) = truth_rewrite(e) {
        e.kind = kind;
    }
    if let K::Not(inner) = &mut e.kind {
        if let K::Not(x) = &mut inner.kind {
            *e = (**x).clone();
        }
    }
}
