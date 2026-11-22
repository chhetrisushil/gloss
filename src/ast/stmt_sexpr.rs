use super::Expr;
use super::sexpr::sexpr;
use super::stmt::{Note, Stmt, StmtKind as S, Target};

pub fn stmt(s: &Stmt) -> String {
    match &s.kind {
        S::Note(note) => note_form(note),
        S::Set { target, value } => target_form("set", target, value),
        S::Add { value, target } => value_target("add", value, target),
        S::Subtract { value, target } => value_target("subtract", value, target),
        S::Multiply { target, by } => target_form("multiply", target, by),
        S::Divide { target, by } => target_form("divide", target, by),
        S::Append { value, list } => format!("(append {} {list})", sexpr(value)),
        S::Show(values) => exprs("show", values),
        S::Ask { question, into } => format!("(ask {} {into})", sexpr(question)),
        S::If {
            branches,
            otherwise,
        } => if_form(branches, otherwise.as_deref()),
        S::ForEachIn { var, list, body } => {
            format!("(for-each {var} {} {})", sexpr(list), block(body))
        }
        S::ForEachRange {
            var,
            from,
            to,
            body,
        } => format!(
            "(for-range {var} {} {} {})",
            sexpr(from),
            sexpr(to),
            block(body)
        ),
        S::RepeatWhile { cond, body } => format!("(repeat {} {})", sexpr(cond), block(body)),
        S::Skip => "skip".into(),
        S::Stop => "stop".into(),
        S::Give(value) => format!("(give {})", sexpr(value)),
        S::Run { feature, inputs } => exprs(&format!("run {feature}"), inputs),
        S::Load(table) => format!("(load {table})"),
        S::Save(table) => format!("(save {table})"),
        S::Clear(table) => format!("(clear {table})"),
        S::Expect(value) => format!("(expect {})", sexpr(value)),
        S::Input(values) => exprs("input", values),
    }
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

fn note_form(note: &Note) -> String {
    match note {
        Note::Line(text) => format!("(note {})", quote(text)),
        Note::Block(lines) => {
            let quoted: Vec<String> = lines.iter().map(|l| quote(l)).collect();
            format!("(note-block {})", quoted.join(" "))
        }
    }
}

fn target(t: &Target) -> String {
    match t {
        Target::Var(name) => name.clone(),
        Target::Field { field, of } => format!("(of {field} {})", target(of)),
        Target::Item { position, of } => format!("(item {} {})", sexpr(position), target(of)),
    }
}

fn target_form(head: &str, t: &Target, value: &Expr) -> String {
    format!("({head} {} {})", target(t), sexpr(value))
}

fn value_target(head: &str, value: &Expr, t: &Target) -> String {
    format!("({head} {} {})", sexpr(value), target(t))
}

fn exprs(head: &str, values: &[Expr]) -> String {
    let parts: String = values.iter().map(|v| format!(" {}", sexpr(v))).collect();
    format!("({head}{parts})")
}

fn block(body: &[Stmt]) -> String {
    let parts: Vec<String> = body.iter().map(stmt).collect();
    format!("[{}]", parts.join(" "))
}

fn if_form(branches: &[(Expr, Vec<Stmt>)], otherwise: Option<&[Stmt]>) -> String {
    let mut parts: Vec<String> = branches
        .iter()
        .map(|(cond, body)| format!("{} {}", sexpr(cond), block(body)))
        .collect();
    if let Some(body) = otherwise {
        parts.push(format!("else {}", block(body)));
    }
    format!("(if {})", parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ExprId, ExprKind as K};
    use crate::number::Number;

    fn e(kind: K) -> Expr {
        Expr {
            id: ExprId(0),
            line: 1,
            kind,
        }
    }
    fn name(s: &str) -> Expr {
        e(K::Name(s.into()))
    }
    fn num(v: i64) -> Expr {
        e(K::Number(Number(v * 10_000)))
    }
    fn st(kind: S) -> Stmt {
        Stmt {
            line: 1,
            blank_before: false,
            kind,
        }
    }
    fn var(s: &str) -> Target {
        Target::Var(s.into())
    }
    fn check(kind: S, want: &str) {
        assert_eq!(stmt(&st(kind)), want);
    }

    #[test]
    fn prints_a_nested_target() {
        let nested = Target::Field {
            field: "amount".into(),
            of: Box::new(Target::Item {
                position: Box::new(num(3)),
                of: Box::new(var("expenses")),
            }),
        };
        check(
            S::Set {
                target: nested,
                value: num(150),
            },
            "(set (of amount (item 3 expenses)) 150)",
        );
    }

    #[test]
    fn prints_every_simple_statement() {
        check(
            S::Note(Note::Line("hi \"x\"".into())),
            "(note \"hi \"\"x\"\"\")",
        );
        check(
            S::Note(Note::Block(vec!["a".into(), "b".into()])),
            "(note-block \"a\" \"b\")",
        );
        let set = S::Set {
            target: var("t"),
            value: num(0),
        };
        check(set, "(set t 0)");
        let add = S::Add {
            value: name("p"),
            target: var("t"),
        };
        check(add, "(add p t)");
        let sub = S::Subtract {
            value: name("d"),
            target: var("t"),
        };
        check(sub, "(subtract d t)");
        let mul = S::Multiply {
            target: var("t"),
            by: num(2),
        };
        check(mul, "(multiply t 2)");
        let div = S::Divide {
            target: var("t"),
            by: name("n"),
        };
        check(div, "(divide t n)");
        let append = S::Append {
            value: name("x"),
            list: "xs".into(),
        };
        check(append, "(append x xs)");
        check(S::Show(vec![num(1), name("b")]), "(show 1 b)");
        let ask = S::Ask {
            question: e(K::Text("Q?".into())),
            into: "a".into(),
        };
        check(ask, "(ask \"Q?\" a)");
        check(S::Skip, "skip");
        check(S::Stop, "stop");
        check(S::Give(name("t")), "(give t)");
        let run = S::Run {
            feature: "f".into(),
            inputs: vec![name("a"), name("b")],
        };
        check(run, "(run f a b)");
        let bare_run = S::Run {
            feature: "go".into(),
            inputs: vec![],
        };
        check(bare_run, "(run go)");
        check(S::Load("t".into()), "(load t)");
        check(S::Save("t".into()), "(save t)");
        check(S::Clear("t".into()), "(clear t)");
        check(S::Expect(name("ok")), "(expect ok)");
        check(S::Input(vec![name("a"), num(2)]), "(input a 2)");
    }

    #[test]
    fn prints_block_statements() {
        let body = vec![st(S::Skip), st(S::Stop)];
        let branches = vec![(name("c"), body.clone()), (name("d"), vec![])];
        check(
            S::If {
                branches: branches.clone(),
                otherwise: Some(vec![st(S::Stop)]),
            },
            "(if c [skip stop] d [] else [stop])",
        );
        check(
            S::If {
                branches,
                otherwise: None,
            },
            "(if c [skip stop] d [])",
        );
        check(
            S::ForEachIn {
                var: "x".into(),
                list: name("xs"),
                body: body.clone(),
            },
            "(for-each x xs [skip stop])",
        );
        check(
            S::ForEachRange {
                var: "i".into(),
                from: num(1),
                to: name("n"),
                body: body.clone(),
            },
            "(for-range i 1 n [skip stop])",
        );
        check(
            S::RepeatWhile {
                cond: name("c"),
                body,
            },
            "(repeat c [skip stop])",
        );
    }
}
