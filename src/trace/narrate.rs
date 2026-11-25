//! Sentences for trace events. So far: the `check` sentence and the map from a line to the
//! condition that starts on it.

use std::collections::HashMap;

use super::event::Value;
use super::value::say;
use crate::ast::print::{expr, expr_with};
use crate::ast::program::Program;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::{Expr, ExprKind, Line};

/// Which statement a condition belongs to: an `if` or `otherwise if`, or a `repeat while`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Branch {
    Block,
    Loop,
}

pub struct Narrator<'p> {
    conditions: HashMap<Line, (&'p Expr, Branch)>,
}

impl<'p> Narrator<'p> {
    /// Maps each line where a condition starts to that condition.
    pub fn new(program: &'p Program) -> Self {
        let mut conditions = HashMap::new();
        for feature in program.features() {
            collect(&feature.body, &mut conditions);
        }
        Narrator { conditions }
    }

    /// The sentence for a `check` event on `line`, or none when no condition starts there.
    pub fn check(&self, line: Line, values: &[(String, Value)], result: bool) -> Option<String> {
        let (cond, branch) = self.conditions.get(&line)?;
        let (runs, skips) = match branch {
            Branch::Block => ("the block runs", "the block is skipped"),
            Branch::Loop => ("the loop runs", "the loop ends"),
        };
        let truth = if result { "yes" } else { "no" };
        let tail = if result { runs } else { skips };
        // A bare name or field read is written `<name> is yes` / `<name> is no`.
        if matches!(cond.kind, ExprKind::Name(_) | ExprKind::FieldOf { .. }) {
            return Some(format!("line {line}: {} is {truth}, so {tail}", expr(cond)));
        }
        let said: HashMap<&str, String> =
            values.iter().map(|(k, v)| (k.as_str(), say(v))).collect();
        let shown = expr_with(cond, &|e| match e.kind {
            ExprKind::Name(_) | ExprKind::FieldOf { .. } => said.get(expr(e).as_str()).cloned(),
            _ => None,
        });
        let is_no = if result { "" } else { " is no" };
        Some(format!("line {line}: {shown}{is_no}, so {tail}"))
    }
}

fn collect<'p>(stmts: &'p [Stmt], map: &mut HashMap<Line, (&'p Expr, Branch)>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::If {
                branches,
                otherwise,
            } => {
                for (cond, body) in branches {
                    map.insert(cond.line, (cond, Branch::Block));
                    collect(body, map);
                }
                collect(otherwise.as_deref().unwrap_or(&[]), map);
            }
            StmtKind::RepeatWhile { cond, body } => {
                map.insert(cond.line, (cond, Branch::Loop));
                collect(body, map);
            }
            StmtKind::ForEachIn { body, .. } | StmtKind::ForEachRange { body, .. } => {
                collect(body, map);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    fn num(s: &str) -> Value {
        Value::Number(s.into())
    }

    /// The sentence for a check on `line` of `body` (inside `feature start`).
    fn sentence(body: &str, line: Line, values: &[(&str, Value)], result: bool) -> String {
        let text = format!("feature start\n{body}");
        let src = Source::new(&text);
        let program = crate::parse_source(&src).expect("parses");
        let values: Vec<(String, Value)> = values
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        Narrator::new(&program)
            .check(line, &values, result)
            .expect("a condition starts there")
    }

    #[test]
    fn a_name_is_replaced_by_its_value() {
        let body = "  set price to 150\n  if price is greater than 100\n    show \"x\"\n";
        assert_eq!(
            sentence(body, 3, &[("price", num("150"))], true),
            "line 3: 150 is greater than 100, so the block runs"
        );
        assert_eq!(
            sentence(body, 3, &[("price", num("50"))], false),
            "line 3: 50 is greater than 100 is no, so the block is skipped"
        );
    }

    #[test]
    fn a_field_read_and_a_name_are_replaced_in_an_and() {
        let body = "  if name of e is \"tea\" and paid\n    show \"x\"\n";
        let values = [
            ("name of e", Value::Text("tea".into())),
            ("paid", Value::Truth(true)),
        ];
        assert_eq!(
            sentence(body, 2, &values, true),
            "line 2: \"tea\" is \"tea\" and yes, so the block runs"
        );
    }

    #[test]
    fn a_single_name_is_written_name_is_yes_or_no() {
        let body = "  if paid\n    show \"x\"\n";
        let values = [("paid", Value::Truth(true))];
        assert_eq!(
            sentence(body, 2, &values, true),
            "line 2: paid is yes, so the block runs"
        );
        let values = [("paid", Value::Truth(false))];
        assert_eq!(
            sentence(body, 2, &values, false),
            "line 2: paid is no, so the block is skipped"
        );
    }

    #[test]
    fn a_call_stays_as_written() {
        // A call with inputs is a `Call` node; only names are ever looked up, so even a
        // value listed under the call's own name is not used.
        let body = "  if f with 1 is greater than 5\n    show \"x\"\n";
        assert_eq!(
            sentence(body, 2, &[("f", num("7"))], true),
            "line 2: f with 1 is greater than 5, so the block runs"
        );
    }

    #[test]
    fn a_call_keeps_its_name_but_its_inputs_are_replaced() {
        let body = "  if f with price is greater than 5\n    show \"x\"\n";
        assert_eq!(
            sentence(body, 2, &[("price", num("150")), ("f", num("7"))], true),
            "line 2: f with 150 is greater than 5, so the block runs"
        );
    }

    #[test]
    fn an_item_position_name_is_replaced_but_the_list_is_not() {
        let body = "  if item i of xs is 9\n    show \"x\"\n";
        assert_eq!(
            sentence(body, 2, &[("i", num("2"))], true),
            "line 2: item 2 of xs is 9, so the block runs"
        );
    }

    #[test]
    fn otherwise_if_and_repeat_while_find_their_own_lines() {
        let body = "  if n is 1\n    show \"a\"\n  otherwise if n is 2\n    show \"b\"\n  repeat while n is less than 2\n    add 1 to n\n";
        let n = |v| [("n", num(v))];
        assert_eq!(
            sentence(body, 4, &n("2"), true),
            "line 4: 2 is 2, so the block runs"
        );
        assert_eq!(
            sentence(body, 6, &n("1"), true),
            "line 6: 1 is less than 2, so the loop runs"
        );
        assert_eq!(
            sentence(body, 6, &n("2"), false),
            "line 6: 2 is less than 2 is no, so the loop ends"
        );
    }

    #[test]
    fn a_line_with_no_condition_has_no_sentence() {
        let src = Source::new("feature start\n  show 1\n");
        let program = crate::parse_source(&src).expect("parses");
        assert_eq!(Narrator::new(&program).check(2, &[], true), None);
    }
}
