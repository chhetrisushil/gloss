use crate::ast::program::{Item, Program};
use crate::ast::stmt::Stmt;
use crate::ast::walk;
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
        let (mut exprs, mut nested) = (Vec::new(), Vec::new());
        walk::parts_mut(&mut s.kind, &mut exprs, &mut nested);
        exprs.into_iter().for_each(expr);
        nested.into_iter().for_each(body);
        if let Some(verb) = set_rewrite(&s.kind) {
            s.kind = verb;
        }
    }
}

fn expr(e: &mut Expr) {
    let before = match &e.kind {
        K::Paren(x) => Some(std::mem::discriminant(&x.kind)),
        _ => None,
    };
    e.children_mut().into_iter().for_each(expr);
    // A rewrite that leaves a bare value inside makes the parentheses redundant.
    let drop_paren = match (&e.kind, before) {
        (K::Paren(x), Some(before)) => {
            let bare = matches!(
                x.kind,
                K::Name(_) | K::Number(_) | K::Text(_) | K::Truth(_) | K::Shown | K::Paren(_)
            );
            bare && std::mem::discriminant(&x.kind) != before
        }
        _ => false,
    };
    if drop_paren {
        if let K::Paren(x) = &e.kind {
            let inner = (**x).clone();
            *e = inner;
        }
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
