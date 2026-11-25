//! Values in sentences: as the program's `show` writes them, shortened for narration (D47).

use super::event::Value;

/// How many items of a list a sentence shows.
const LIST_ITEMS: usize = 3;
/// How many characters of a text a sentence shows.
const TEXT_CHARS: usize = 60;

/// The value as `show` writes it, except that a text is always in quotes, a list of more than
/// three items shows its first three and then `… (N items)`, and a text of more than sixty
/// characters keeps sixty and ends with `…` (inside its quotes).
pub fn say(v: &Value) -> String {
    match v {
        Value::List(items) => say_list(items),
        Value::Record { .. } => say_record(v),
        scalar => say_scalar(scalar),
    }
}

fn say_scalar(v: &Value) -> String {
    match v {
        Value::Number(n) => n.clone(),
        Value::Truth(b) => if *b { "yes" } else { "no" }.into(),
        Value::Text(t) => quoted(t),
        Value::List(_) | Value::Record { .. } => unreachable!("records hold only scalars"),
    }
}

/// A text in quotes, each `"` doubled, cut when it is long.
fn quoted(t: &str) -> String {
    let (kept, cut) = match t.char_indices().nth(TEXT_CHARS) {
        Some((end, _)) => (&t[..end], "…"),
        None => (t, ""),
    };
    format!("\"{}{cut}\"", kept.replace('"', "\"\""))
}

fn say_record(v: &Value) -> String {
    let Value::Record { name, fields } = v else {
        unreachable!("only called on records");
    };
    if fields.is_empty() {
        return name.clone();
    }
    let fields: Vec<String> = fields
        .iter()
        .map(|(field, value)| format!("{field} {}", say_scalar(value)))
        .collect();
    format!("{name} with {}", fields.join(", "))
}

fn say_list(items: &[Value]) -> String {
    // The item type of an empty list is not in the trace, so it cannot be named.
    if items.is_empty() {
        return "empty list".into();
    }
    let mut shown: Vec<String> = items.iter().take(LIST_ITEMS).map(say_item).collect();
    if items.len() > LIST_ITEMS {
        shown.push(format!("… ({} items)", items.len()));
    }
    format!("list of {}", shown.join(", "))
}

/// A list item: a record is in parentheses.
fn say_item(v: &Value) -> String {
    match v {
        Value::Record { .. } => format!("({})", say_record(v)),
        other => say(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check;
    use crate::irsim::run;
    use crate::lower::{lower, lower_trace};
    use crate::source::Source;
    use crate::trace::event::{Event, decode};

    fn n(s: &str) -> Value {
        Value::Number(s.into())
    }

    /// What the program's `show` of its variable `v` writes, and `say` of the value its
    /// `set` event for `v` holds (the last one).
    fn show_and_say(program: &str) -> (String, String) {
        let src = Source::new(program);
        let parsed = crate::parse_source(&src).expect("parses");
        let types = check(&parsed, &src).types.expect("checks clean");
        let plain = lower(&parsed, &types, &src).expect("lowers");
        let shown = String::from_utf8(run(&plain, "_start", b"").stdout).unwrap();
        let traced = lower_trace(&parsed, &types, &src).expect("lowers");
        let events = String::from_utf8(run(&traced, "_start", b"").stdout).unwrap();
        let said = events
            .lines()
            .filter_map(|l| match decode(l).expect("an event") {
                Event::Set { name, to, .. } if name == "v" => Some(say(&to)),
                _ => None,
            })
            .next_back()
            .expect("a set event for v");
        (shown.trim_end_matches('\n').to_string(), said)
    }

    const RECORD: &str = "record P\n  name as text\n  age as number\n\n";

    #[test]
    fn say_agrees_with_show() {
        let programs = [
            "feature start\n  set v to 7.5\n  show v\n".to_string(),
            "feature start\n  set v to yes\n  show v\n".to_string(),
            format!(
                "{RECORD}feature start\n  set v to P with name \"say \"\"hi\"\"\", age 9\n  show v\n"
            ),
            format!(
                "{RECORD}feature start\n  set v to list of (P with name \"a\", age 1), (P with name \"b\"\"c\", age 2)\n  show v\n"
            ),
        ];
        for program in programs {
            let (shown, said) = show_and_say(&program);
            assert_eq!(said, shown, "{program}");
        }
        // A top-level `show` of a text writes no quotes; a sentence always has them.
        let (shown, said) =
            show_and_say("feature start\n  set v to \"say \"\"hi\"\"\"\n  show v\n");
        assert_eq!(
            (shown.as_str(), said.as_str()),
            ("say \"hi\"", "\"say \"\"hi\"\"\"")
        );
    }

    #[test]
    fn a_long_list_is_shortened() {
        let list = Value::List((1..=5).map(|i| n(&i.to_string())).collect());
        assert_eq!(say(&list), "list of 1, 2, 3, … (5 items)");
        let three = Value::List(vec![n("1"), n("2"), n("3")]);
        assert_eq!(say(&three), "list of 1, 2, 3");
    }

    #[test]
    fn a_long_text_is_cut() {
        let sixty = "é".repeat(60);
        assert_eq!(say(&Value::Text(sixty.clone())), format!("\"{sixty}\""));
        let cut = say(&Value::Text(format!("{sixty}é")));
        assert_eq!(cut, format!("\"{sixty}…\""));
        assert_eq!(cut.chars().count(), 63);
    }
}
