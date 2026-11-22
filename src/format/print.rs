use crate::ast::Expr;
use crate::ast::print::expr;
use crate::ast::stmt::{Note, Stmt, StmtKind as S, Target};

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

/// A statement list as lines at `level`, with one blank line before each
/// statement that had one, except at the top of the list.
pub fn stmts(body: &[Stmt], level: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for s in body {
        if s.blank_before && !lines.is_empty() {
            lines.push(String::new());
        }
        lines.extend(stmt(s, level));
    }
    lines
}

/// One statement, with its block, as lines at `level`.
pub fn stmt(s: &Stmt, level: usize) -> Vec<String> {
    let pad = indent(level);
    let head = |text: String| format!("{pad}{text}");
    match &s.kind {
        S::Note(note) => note_lines(note, level),
        S::If {
            branches,
            otherwise,
        } => {
            let mut lines = Vec::new();
            for (i, (cond, body)) in branches.iter().enumerate() {
                let word = if i == 0 { "if" } else { "otherwise if" };
                lines.push(head(format!("{word} {}", expr(cond))));
                lines.extend(stmts(body, level + 1));
            }
            if let Some(body) = otherwise {
                lines.push(head("otherwise".into()));
                lines.extend(stmts(body, level + 1));
            }
            lines
        }
        S::ForEachIn { var, list, body } => block(
            head(format!("for each {var} in {}", expr(list))),
            body,
            level,
        ),
        S::ForEachRange {
            var,
            from,
            to,
            body,
        } => {
            let text = format!("for each {var} from {} to {}", expr(from), expr(to));
            block(head(text), body, level)
        }
        S::RepeatWhile { cond, body } => {
            block(head(format!("repeat while {}", expr(cond))), body, level)
        }
        _ => vec![head(simple(&s.kind))],
    }
}

fn block(first: String, body: &[Stmt], level: usize) -> Vec<String> {
    let mut lines = vec![first];
    lines.extend(stmts(body, level + 1));
    lines
}

/// `note` lines for a top-level, record or statement note at `level`.
pub fn note_lines(note: &Note, level: usize) -> Vec<String> {
    let pad = indent(level);
    match note {
        Note::Line(text) => vec![format!("{pad}note {text}")],
        Note::Block(lines) => {
            let mut out = vec![format!("{pad}note")];
            for line in lines {
                let text = if line.is_empty() {
                    String::new()
                } else {
                    format!("{pad}  {line}")
                };
                out.push(text);
            }
            out
        }
    }
}

fn target(t: &Target) -> String {
    match t {
        Target::Var(name) => name.clone(),
        Target::Field { field, of } => format!("{field} of {}", target(of)),
        Target::Item { position, of } => format!("item {} of {}", expr(position), target(of)),
    }
}

fn list(values: &[Expr]) -> String {
    values.iter().map(expr).collect::<Vec<_>>().join(", ")
}

fn simple(kind: &S) -> String {
    match kind {
        S::Set { target: t, value } => format!("set {} to {}", target(t), expr(value)),
        S::Add { value, target: t } => format!("add {} to {}", expr(value), target(t)),
        S::Subtract { value, target: t } => format!("subtract {} from {}", expr(value), target(t)),
        S::Multiply { target: t, by } => format!("multiply {} by {}", target(t), expr(by)),
        S::Divide { target: t, by } => format!("divide {} by {}", target(t), expr(by)),
        S::Append { value, list } => format!("append {} to {list}", expr(value)),
        S::Show(values) => format!("show {}", list(values)),
        S::Input(values) => format!("input {}", list(values)),
        S::Ask { question, into } => format!("ask {} into {into}", expr(question)),
        S::Skip => "skip".into(),
        S::Stop => "stop".into(),
        S::Give(value) => format!("give {}", expr(value)),
        S::Expect(value) => format!("expect {}", expr(value)),
        S::Run { feature, inputs } if inputs.is_empty() => format!("run {feature}"),
        S::Run { feature, inputs } => format!("run {feature} with {}", list(inputs)),
        S::Load(name) => format!("load {name}"),
        S::Save(name) => format!("save {name}"),
        S::Clear(name) => format!("clear {name}"),
        S::Note(_)
        | S::If { .. }
        | S::ForEachIn { .. }
        | S::ForEachRange { .. }
        | S::RepeatWhile { .. } => unreachable!("handled by stmt"),
    }
}

#[cfg(test)]
mod tests {
    use super::stmts;
    use crate::parser::parse_stmts_str;

    fn fmt(text: &str) -> String {
        let lines = stmts(&parse_stmts_str(text).unwrap(), 0);
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    #[test]
    fn every_statement_round_trips() {
        let text = "note one line\nnote\n  block\n\n    deeper\nset x to 1 + 2\nset item 2 of xs to 3\n\
add 1 to total\nsubtract 2 from total\nmultiply total by 3\ndivide total by 4\n\
append P with a 1, b \"q\"\"r\" to ps\nshow 1, \"a\"\ninput \"x\", \"y\"\nask \"q\" into a\n\
if a is 1\n  skip\n\n  stop\notherwise if a is 2\n  show 2\notherwise\n  show 3\n\
for each x in xs\n  show x\nfor each i from 1 to 3\n  show i\n\
repeat while a is not number\n  ask \"q\" into a\ngive a\nrun f\nrun f with 1, a\n\
load t\nsave t\nclear t\nexpect a is 1\n";
        assert_eq!(fmt(text), text);
    }

    #[test]
    fn spacing_is_normalised() {
        let messy = "set  x   to 1+2\n\n\n\nshow x ,\"a\"\n";
        assert_eq!(fmt(messy), "set x to 1 + 2\n\nshow x, \"a\"\n");
    }
}
