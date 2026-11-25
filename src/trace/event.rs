//! Typed events: what each line of a trace holds, decoded from its JSON.

use super::json::{Json, parse};

/// A typed value as the traced program wrote it. Numbers keep their `show` form.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(String),
    Text(String),
    Truth(bool),
    List(Vec<Value>),
    Record {
        name: String,
        fields: Vec<(String, Value)>,
    },
}

/// The pairs of an object, or an error naming `what`.
pub(super) fn pairs<'a>(json: &'a Json, what: &str) -> Result<&'a [(String, Json)], String> {
    match json {
        Json::Object(pairs) => Ok(pairs),
        _ => Err(format!("{what} must be an object")),
    }
}

/// The value under `key`, or an error naming it.
pub(super) fn get<'a>(pairs: &'a [(String, Json)], key: &str) -> Result<&'a Json, String> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("missing key \"{key}\""))
}

/// The text under `key`, or an error naming it.
pub(super) fn text(pairs: &[(String, Json)], key: &str) -> Result<String, String> {
    match get(pairs, key)? {
        Json::Str(s) => Ok(s.clone()),
        _ => Err(format!("key \"{key}\" must be text")),
    }
}

/// Reads a typed value: `{"number":"3"}`, `{"text":"a"}`, `{"truth":"yes"}`, `{"list":[…]}`
/// or `{"record":"Name","fields":{…}}`.
pub fn decode_value(json: &Json) -> Result<Value, String> {
    let pairs = pairs(json, "a value")?;
    let Some((kind, body)) = pairs.first() else {
        return Err("a value has no type key".to_string());
    };
    match kind.as_str() {
        "number" => text(pairs, "number").map(Value::Number),
        "text" => text(pairs, "text").map(Value::Text),
        "truth" => match text(pairs, "truth")?.as_str() {
            "yes" => Ok(Value::Truth(true)),
            "no" => Ok(Value::Truth(false)),
            _ => Err("key \"truth\" must be yes or no".to_string()),
        },
        "list" => match body {
            Json::Array(items) => items
                .iter()
                .map(decode_value)
                .collect::<Result<_, _>>()
                .map(Value::List),
            _ => Err("key \"list\" must be an array".to_string()),
        },
        "record" => {
            let name = text(pairs, "record")?;
            let fields = decode_fields(get(pairs, "fields")?)?;
            Ok(Value::Record { name, fields })
        }
        other => Err(format!("unknown value type \"{other}\"")),
    }
}

/// Reads an object whose values are typed values, in key order.
pub(super) fn decode_fields(json: &Json) -> Result<Vec<(String, Value)>, String> {
    pairs(json, "fields")?
        .iter()
        .map(|(k, v)| Ok((k.clone(), decode_value(v)?)))
        .collect()
}

/// What a `file` event did to its table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileAction {
    Load,
    Save,
    Clear,
}

/// One line of a trace.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Set {
        line: u32,
        name: String,
        item: Option<i64>,
        field: Option<String>,
        from: Option<Value>,
        to: Value,
    },
    Check {
        line: u32,
        condition: String,
        values: Vec<(String, Value)>,
        result: bool,
    },
    Loop {
        line: u32,
        name: String,
        turn: i64,
        value: Value,
    },
    Enter {
        line: u32,
        feature: String,
        inputs: Vec<(String, Value)>,
    },
    Leave {
        line: u32,
        feature: String,
        result: Option<Value>,
    },
    Show {
        line: u32,
        text: String,
    },
    Ask {
        line: u32,
        name: String,
        question: String,
        answer: String,
    },
    File {
        line: u32,
        table: String,
        file: String,
        action: FileAction,
        rows: i64,
    },
    Fail {
        line: u32,
        failure: i64,
        message: String,
    },
}

/// The integer under `key`, or an error naming it.
fn int(pairs: &[(String, Json)], key: &str) -> Result<i64, String> {
    match get(pairs, key)? {
        Json::Int(n) => Ok(*n),
        _ => Err(format!("key \"{key}\" must be an integer")),
    }
}

/// The value under `key`, if the event has one.
fn optional<'a>(pairs: &'a [(String, Json)], key: &str) -> Option<&'a Json> {
    get(pairs, key).ok()
}

/// A `yes` or `no` under `key`.
fn truth(pairs: &[(String, Json)], key: &str) -> Result<bool, String> {
    match text(pairs, key)?.as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("key \"{key}\" must be yes or no")),
    }
}

/// Reads one trace line as a typed event.
pub fn decode(line: &str) -> Result<Event, String> {
    let json = parse(line)?;
    let p = pairs(&json, "an event")?;
    let at = u32::try_from(int(p, "line")?).map_err(|_| "key \"line\" is out of range")?;
    let line = at;
    Ok(match text(p, "event")?.as_str() {
        "set" => Event::Set {
            line,
            name: text(p, "name")?,
            item: optional(p, "item").map(|_| int(p, "item")).transpose()?,
            field: optional(p, "field").map(|_| text(p, "field")).transpose()?,
            from: optional(p, "from").map(decode_value).transpose()?,
            to: decode_value(get(p, "to")?)?,
        },
        "check" => Event::Check {
            line,
            condition: text(p, "condition")?,
            values: decode_fields(get(p, "values")?)?,
            result: truth(p, "result")?,
        },
        "loop" => Event::Loop {
            line,
            name: text(p, "name")?,
            turn: int(p, "turn")?,
            value: decode_value(get(p, "value")?)?,
        },
        "enter" => Event::Enter {
            line,
            feature: text(p, "feature")?,
            inputs: decode_fields(get(p, "inputs")?)?,
        },
        "leave" => Event::Leave {
            line,
            feature: text(p, "feature")?,
            result: optional(p, "result").map(decode_value).transpose()?,
        },
        "show" => Event::Show {
            line,
            text: text(p, "text")?,
        },
        "ask" => Event::Ask {
            line,
            name: text(p, "name")?,
            question: text(p, "question")?,
            answer: text(p, "answer")?,
        },
        "file" => Event::File {
            line,
            table: text(p, "table")?,
            file: text(p, "file")?,
            action: match text(p, "action")?.as_str() {
                "load" => FileAction::Load,
                "save" => FileAction::Save,
                "clear" => FileAction::Clear,
                _ => return Err("key \"action\" must be load, save or clear".to_string()),
            },
            rows: int(p, "rows")?,
        },
        "fail" => Event::Fail {
            line,
            failure: int(p, "failure")?,
            message: text(p, "message")?,
        },
        other => return Err(format!("unknown event \"{other}\"")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::json::parse;

    fn value(text: &str) -> Result<Value, String> {
        decode_value(&parse(text).unwrap())
    }

    fn num(s: &str) -> Value {
        Value::Number(s.to_string())
    }

    #[test]
    fn decodes_every_kind_of_value() {
        assert_eq!(value(r#"{"number":"7.5"}"#), Ok(num("7.5")));
        assert_eq!(
            value(r#"{"text":"a \"b\""}"#),
            Ok(Value::Text("a \"b\"".into()))
        );
        assert_eq!(value(r#"{"truth":"yes"}"#), Ok(Value::Truth(true)));
        assert_eq!(value(r#"{"truth":"no"}"#), Ok(Value::Truth(false)));
        assert_eq!(
            value(r#"{"list":[{"number":"1"},{"number":"2"}]}"#),
            Ok(Value::List(vec![num("1"), num("2")]))
        );
        assert_eq!(value(r#"{"list":[]}"#), Ok(Value::List(vec![])));
        let record = r#"{"record":"Row","fields":{"name":{"text":"a"},"n":{"number":"1"}}}"#;
        assert_eq!(
            value(record),
            Ok(Value::Record {
                name: "Row".into(),
                fields: vec![
                    ("name".into(), Value::Text("a".into())),
                    ("n".into(), num("1"))
                ],
            })
        );
    }

    #[test]
    fn a_bad_value_is_an_error_that_names_the_key() {
        let err = |text| value(text).unwrap_err();
        assert_eq!(err(r#"{"number":3}"#), "key \"number\" must be text");
        assert_eq!(
            err(r#"{"truth":"maybe"}"#),
            "key \"truth\" must be yes or no"
        );
        assert_eq!(err(r#"{"list":3}"#), "key \"list\" must be an array");
        assert_eq!(err(r#"{"record":"R"}"#), "missing key \"fields\"");
        assert_eq!(
            err(r#"{"record":"R","fields":{"a":{"x":"1"}}}"#),
            "unknown value type \"x\""
        );
        assert_eq!(err("{}"), "a value has no type key");
        assert_eq!(err("3"), "a value must be an object");
    }

    #[test]
    fn every_line_of_every_recorded_trace_decodes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_cases");
        let (mut files, mut lines) = (0, 0);
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "trace") {
                files += 1;
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    lines += 1;
                    decode(line).unwrap_or_else(|e| panic!("{}: {line}: {e}", path.display()));
                }
            }
        }
        assert!(files >= 44 && lines > files);
    }
}
