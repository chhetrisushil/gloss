//! Sentences for trace events: one per event, indented by call depth, and the map from a line
//! to the condition that starts on it.

use std::collections::HashMap;
use std::io::{self, Write};

use super::event::{Event, FileAction, Value, decode};
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
    /// Open calls below `start`: each sentence is indented 2 spaces per call.
    depth: usize,
}

impl<'p> Narrator<'p> {
    /// Maps each line where a condition starts to that condition.
    pub fn new(program: &'p Program) -> Self {
        let mut conditions = HashMap::new();
        for feature in program.features() {
            collect(&feature.body, &mut conditions);
        }
        Narrator {
            conditions,
            depth: 0,
        }
    }

    /// Narrates one trace line: a `show` text on `out`, a `fail` on `err`, anything else as a
    /// sentence on `out`. The inner error is a line that is not an event.
    pub fn line(
        &mut self,
        event_line: &str,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> io::Result<Result<(), String>> {
        let event = match decode(event_line) {
            Ok(event) => event,
            Err(e) => return Ok(Err(e)),
        };
        let enters = matches!(event, Event::Enter { .. });
        let sentence = match event {
            Event::Show { text, .. } => {
                writeln!(out, "{text}")?;
                return Ok(Ok(()));
            }
            Event::Fail {
                line,
                failure,
                message,
                ..
            } => {
                writeln!(err, "line {line}: failure {failure}: {message}")?;
                return Ok(Ok(()));
            }
            Event::Set {
                line,
                name,
                item,
                field,
                from,
                to,
            } => set_sentence(line, &name, item, field.as_deref(), from.is_some(), &to),
            Event::Check {
                line,
                values,
                result,
                ..
            } => self.check(line, &values, result),
            Event::Loop {
                line, name, value, ..
            } => Some(format!("line {line}: {name} is {}", say(&value))),
            Event::Enter {
                line,
                feature,
                inputs,
            } => {
                let with: Vec<String> = inputs
                    .iter()
                    .map(|(k, v)| format!("{k} = {}", say(v)))
                    .collect();
                let with = if with.is_empty() {
                    String::new()
                } else {
                    format!(" with {}", with.join(", "))
                };
                Some(format!("line {line}: calling {feature}{with}"))
            }
            Event::Leave {
                line,
                feature,
                result,
            } => {
                // The callee's own sentences are one level deeper than its enter and leave.
                self.depth = self.depth.saturating_sub(1);
                Some(match result {
                    Some(v) => format!("line {line}: {feature} gives {}", say(&v)),
                    None => format!("line {line}: {feature} ends"),
                })
            }
            Event::Ask {
                line, name, answer, ..
            } => Some(format!(
                "line {line}: the answer {} goes into {name}",
                say(&Value::Text(answer))
            )),
            Event::File {
                line,
                table,
                file,
                action,
                rows,
            } => {
                let s = if rows == 1 { "" } else { "s" };
                Some(match action {
                    FileAction::Load => {
                        format!("line {line}: loaded {table} from {file} ({rows} row{s})")
                    }
                    FileAction::Save => {
                        format!("line {line}: saved {table} to {file} ({rows} row{s})")
                    }
                    FileAction::Clear => format!("line {line}: cleared {table}"),
                })
            }
        };
        if let Some(sentence) = sentence {
            writeln!(out, "{}{sentence}", "  ".repeat(self.depth))?;
        }
        if enters {
            self.depth += 1;
        }
        Ok(Ok(()))
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

/// The sentence for a `set` event. An item with no old value and no field is an append.
fn set_sentence(
    line: u32,
    name: &str,
    item: Option<i64>,
    field: Option<&str>,
    had_old: bool,
    to: &Value,
) -> Option<String> {
    let value = say(to);
    let target = match (item, field) {
        (None, None) if !had_old => return Some(format!("line {line}: {name} is {value}")),
        (None, _) => name.to_string(),
        (Some(n), None) if !had_old => {
            let s = if n == 1 { "" } else { "s" };
            return Some(format!(
                "line {line}: {value} is appended to {name} (now {n} item{s})"
            ));
        }
        (Some(n), None) => format!("item {n} of {name}"),
        (Some(n), Some(f)) => format!("{f} of item {n} of {name}"),
    };
    let target = match (item, field) {
        (None, Some(f)) => format!("{f} of {target}"),
        _ => target,
    };
    Some(format!("line {line}: {target} becomes {value}"))
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

    /// The narration of `lines`: (stdout, stderr).
    fn narrate(body: &str, lines: &[&str]) -> (String, String) {
        let text = format!("feature start\n{body}");
        let src = Source::new(&text);
        let program = crate::parse_source(&src).expect("parses");
        let mut n = Narrator::new(&program);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        for l in lines {
            n.line(l, &mut out, &mut err).unwrap().unwrap();
        }
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    const N: &str = "{\"number\":\"";

    #[test]
    fn show_and_fail_are_printed_unindented_on_their_streams() {
        let (out, err) = narrate(
            "  show 1\n",
            &[
                r#"{"line":2,"event":"show","text":"hi \"there\""}"#,
                r#"{"line":2,"event":"fail","failure":1,"message":"dividing by zero (n is 0)"}"#,
            ],
        );
        assert_eq!(out, "hi \"there\"\n");
        assert_eq!(err, "line 2: failure 1: dividing by zero (n is 0)\n");
    }

    #[test]
    fn set_sentences() {
        let set = |extra: &str, to: &str| {
            let l = format!(r#"{{"line":4,"event":"set","name":"x"{extra},"to":{to}}}"#);
            narrate("  show 1\n", &[l.as_str()]).0
        };
        let from = format!(r#","from":{N}1"}}"#);
        let (to, five) = (format!(r#"{N}12"}}"#), format!(r#"{N}5"}}"#));
        assert_eq!(set("", &to), "line 4: x is 12\n");
        assert_eq!(set(&from, &to), "line 4: x becomes 12\n");
        assert_eq!(
            set(&format!(r#","field":"amount"{from}"#), &to),
            "line 4: amount of x becomes 12\n"
        );
        assert_eq!(
            set(&format!(r#","item":2{from}"#), &to),
            "line 4: item 2 of x becomes 12\n"
        );
        assert_eq!(
            set(&format!(r#","item":2,"field":"amount"{from}"#), &to),
            "line 4: amount of item 2 of x becomes 12\n"
        );
        assert_eq!(
            set(r#","item":1"#, &five),
            "line 4: 5 is appended to x (now 1 item)\n"
        );
        assert_eq!(
            set(r#","item":4"#, &five),
            "line 4: 5 is appended to x (now 4 items)\n"
        );
    }

    #[test]
    fn loop_ask_file_and_calls_are_narrated_with_a_staircase() {
        let lines = [
            r#"{"line":9,"event":"loop","name":"price","turn":1,"value":{"number":"150"}}"#,
            r#"{"line":8,"event":"enter","feature":"gcd","inputs":{"a":{"number":"48"},"b":{"number":"36"}}}"#,
            r#"{"line":3,"event":"enter","feature":"report","inputs":{}}"#,
            r#"{"line":4,"event":"ask","name":"item","question":"q","answer":"a \"b\""}"#,
            r#"{"line":3,"event":"leave","feature":"report"}"#,
            r#"{"line":8,"event":"leave","feature":"gcd","result":{"number":"12"}}"#,
            r#"{"line":2,"event":"file","table":"expenses","file":"expenses.csv","action":"load","rows":12}"#,
            r#"{"line":3,"event":"file","table":"expenses","file":"expenses.csv","action":"save","rows":1}"#,
            r#"{"line":4,"event":"file","table":"expenses","file":"expenses.csv","action":"clear","rows":0}"#,
        ];
        let (out, err) = narrate("  show 1\n", &lines);
        assert_eq!(
            out,
            "line 9: price is 150\n\
             line 8: calling gcd with a = 48, b = 36\n\
             \x20 line 3: calling report\n\
             \x20   line 4: the answer \"a \"\"b\"\"\" goes into item\n\
             \x20 line 3: report ends\n\
             line 8: gcd gives 12\n\
             line 2: loaded expenses from expenses.csv (12 rows)\n\
             line 3: saved expenses to expenses.csv (1 row)\n\
             line 4: cleared expenses\n"
        );
        assert_eq!(err, "");
    }

    #[test]
    fn a_check_event_is_narrated_and_a_bad_line_is_an_error() {
        let body = "  if paid\n    show 1\n";
        let (out, _) = narrate(
            body,
            &[
                r#"{"line":2,"event":"check","condition":"paid","values":{"paid":{"truth":"no"}},"result":"no"}"#,
            ],
        );
        assert_eq!(out, "line 2: paid is no, so the block is skipped\n");
        let src = Source::new("feature start\n  show 1\n");
        let program = crate::parse_source(&src).unwrap();
        let r = Narrator::new(&program).line("nope", &mut Vec::new(), &mut Vec::new());
        assert!(r.unwrap().is_err());
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
