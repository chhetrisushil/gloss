use std::collections::HashSet;

use crate::ast::stmt::{Stmt, StmtKind, Target};
use crate::ast::walk::for_each_expr;
use crate::ast::{Expr, ExprKind, Line};

/// Where a variable is first set, and the outermost if or loop around that set.
#[derive(Clone, Copy)]
pub struct Origin {
    pub line: Line,
    pub within: Option<(&'static str, Line)>,
}

impl Origin {
    /// Where the variable is first set, and where to set it instead.
    pub fn describe(&self) -> (String, String) {
        match self.within {
            Some((word, at)) => (
                format!("inside the {word} on line {at}"),
                format!("before the {word}"),
            ),
            None => (format!("on line {}", self.line), "before this line".into()),
        }
    }
}

/// Which statements end a block early.
#[derive(Clone, Copy, PartialEq)]
pub enum Exits {
    /// Only a give, which ends the feature.
    Give,
    /// A give, and a stop or skip, which end the pass of a loop.
    Any,
}

/// True when the code after a body is never reached from its end: a statement of the body
/// gives (or, for `Exits::Any`, stops or skips), or is an if with an otherwise whose every
/// branch does.
pub fn diverges(body: &[Stmt], exits: Exits) -> bool {
    body.iter().any(|s| match &s.kind {
        StmtKind::Give(_) => true,
        StmtKind::Stop | StmtKind::Skip => exits == Exits::Any,
        StmtKind::If {
            branches,
            otherwise: Some(other),
        } => branches.iter().all(|(_, b)| diverges(b, exits)) && diverges(other, exits),
        _ => false,
    })
}

/// Finds each variable's first `set` or `ask`, in source order.
pub fn firsts(
    body: &[Stmt],
    within: Option<(&'static str, Line)>,
    out: &mut Vec<(String, Origin)>,
) {
    let mut seen = out.iter().map(|(n, _)| n.clone()).collect();
    firsts_seen(body, within, out, &mut seen);
}

/// `seen` holds the names in `out`, so a name is looked up without scanning `out`.
fn firsts_seen(
    body: &[Stmt],
    within: Option<(&'static str, Line)>,
    out: &mut Vec<(String, Origin)>,
    seen: &mut HashSet<String>,
) {
    for stmt in body {
        let here = |word| within.or(Some((word, stmt.line)));
        let mut note = |name: &str| {
            if seen.insert(name.into()) {
                out.push((
                    name.into(),
                    Origin {
                        line: stmt.line,
                        within,
                    },
                ));
            }
        };
        match &stmt.kind {
            StmtKind::Set {
                target: Target::Var(name),
                ..
            } => note(name),
            StmtKind::Ask { into, .. } => note(into),
            StmtKind::If {
                branches,
                otherwise,
            } => {
                for (_, b) in branches {
                    firsts_seen(b, here("if"), out, seen);
                }
                firsts_seen(
                    otherwise.as_deref().unwrap_or_default(),
                    here("if"),
                    out,
                    seen,
                );
            }
            StmtKind::ForEachIn { var, body, .. } | StmtKind::ForEachRange { var, body, .. } => {
                // Inside the loop the variable is the loop's own, not a new name.
                let (kept, had) = (out.len(), seen.contains(var.as_str()));
                firsts_seen(body, here("loop"), out, seen);
                if !had {
                    seen.remove(var.as_str());
                }
                let mut at = 0;
                out.retain(|(n, _)| {
                    at += 1;
                    at <= kept || n != var
                });
            }
            StmtKind::RepeatWhile { body, .. } => firsts_seen(body, here("loop"), out, seen),
            _ => {}
        }
    }
}

/// Every variable name an expression reads.
pub fn reads<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
    for_each_expr(e, &mut |x| {
        if let ExprKind::Name(n) = &x.kind {
            out.push(n);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    fn body(text: &str) -> Vec<Stmt> {
        let program = crate::parse_source(&Source::new(text)).unwrap();
        program.features().next().unwrap().body.clone()
    }

    #[test]
    fn first_sets_know_their_if_or_loop() {
        let b = body(
            "feature start\n  set a to 1\n  if yes\n    set b to 1\n    set a to 2\n  for each i from 1 to 2\n    ask \"q\" into c\n",
        );
        let mut out = Vec::new();
        firsts(&b, None, &mut out);
        let got: Vec<_> = out
            .iter()
            .map(|(n, o)| (n.as_str(), o.line, o.within))
            .collect();
        assert_eq!(out[1].1.describe().1, "before the if");
        assert_eq!(out[0].1.describe().0, "on line 2");
        assert_eq!(
            got,
            [
                ("a", 2, None),
                ("b", 4, Some(("if", 3))),
                ("c", 7, Some(("loop", 6)))
            ]
        );
    }

    #[test]
    fn bodies_that_diverge() {
        let b = body(
            "feature start\n  if yes\n    give 1\n  otherwise\n    stop\n  if yes\n    give 1\n",
        );
        assert!(diverges(&b[..1], Exits::Any));
        assert!(!diverges(&b[..1], Exits::Give));
        assert!(!diverges(&b[1..], Exits::Any));
        assert!(!diverges(&[], Exits::Any));
        let early = body("feature start\n  give 1\n  show 2\n");
        assert!(diverges(&early, Exits::Give));
    }

    #[test]
    fn reads_find_every_name() {
        let b = body("feature start\n  show a + (b * 2), count of c, (f with d)\n");
        let StmtKind::Show(items) = &b[0].kind else {
            panic!("not a show");
        };
        let mut names = Vec::new();
        items.iter().for_each(|e| reads(e, &mut names));
        assert_eq!(names, ["a", "b", "c", "d"]);
    }
}
