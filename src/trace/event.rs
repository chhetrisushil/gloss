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
    as_text(get(pairs, key)?, key)
}

fn as_text(json: &Json, key: &str) -> Result<String, String> {
    match json {
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
        "truth" => truth(pairs, "truth").map(Value::Truth),
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
    as_int(get(pairs, key)?, key)
}

fn as_int(json: &Json, key: &str) -> Result<i64, String> {
    match json {
        Json::Int(n) => Ok(*n),
        _ => Err(format!("key \"{key}\" must be an integer")),
    }
}

/// Reads the optional key `key` with `read`; a key that is there but wrong is an error.
fn optional<T>(
    pairs: &[(String, Json)],
    key: &str,
    read: impl FnOnce(&Json) -> Result<T, String>,
) -> Result<Option<T>, String> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| read(v))
        .transpose()
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
    let line = u32::try_from(int(p, "line")?).map_err(|_| "key \"line\" is out of range")?;
    Ok(match text(p, "event")?.as_str() {
        "set" => Event::Set {
            line,
            name: text(p, "name")?,
            item: optional(p, "item", |j| as_int(j, "item"))?,
            field: optional(p, "field", |j| as_text(j, "field"))?,
            from: optional(p, "from", decode_value)?,
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
            result: optional(p, "result", decode_value)?,
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

    fn num(s: &str) -> Value {
        Value::Number(s.to_string())
    }

    #[test]
    fn decodes_each_kind_of_event() {
        let n = |s: &str| s.to_string();
        let set = r#"{"line":3,"event":"set","name":"rows","item":2,"field":"n","from":{"number":"1"},"to":{"number":"2"}}"#;
        assert_eq!(
            decode(set),
            Ok(Event::Set {
                line: 3,
                name: n("rows"),
                item: Some(2),
                field: Some(n("n")),
                from: Some(num("1")),
                to: num("2"),
            })
        );
        let first = r#"{"line":1,"event":"set","name":"t","to":{"truth":"yes"}}"#;
        assert_eq!(
            decode(first),
            Ok(Event::Set {
                line: 1,
                name: n("t"),
                item: None,
                field: None,
                from: None,
                to: Value::Truth(true)
            })
        );
        let check = r#"{"line":4,"event":"check","condition":"a > 1","values":{"a":{"number":"2"}},"result":"no"}"#;
        assert_eq!(
            decode(check),
            Ok(Event::Check {
                line: 4,
                condition: n("a > 1"),
                values: vec![(n("a"), num("2"))],
                result: false
            })
        );
        let l = r#"{"line":5,"event":"loop","name":"p","turn":1,"value":{"number":"9"}}"#;
        assert_eq!(
            decode(l),
            Ok(Event::Loop {
                line: 5,
                name: n("p"),
                turn: 1,
                value: num("9")
            })
        );
        let enter = r#"{"line":6,"event":"enter","feature":"f","inputs":{}}"#;
        assert_eq!(
            decode(enter),
            Ok(Event::Enter {
                line: 6,
                feature: n("f"),
                inputs: vec![]
            })
        );
        let leave = r#"{"line":6,"event":"leave","feature":"f","result":{"number":"1"}}"#;
        assert_eq!(
            decode(leave),
            Ok(Event::Leave {
                line: 6,
                feature: n("f"),
                result: Some(num("1"))
            })
        );
        let bare = r#"{"line":6,"event":"leave","feature":"f"}"#;
        assert_eq!(
            decode(bare),
            Ok(Event::Leave {
                line: 6,
                feature: n("f"),
                result: None
            })
        );
        assert_eq!(
            decode(r#"{"line":7,"event":"show","text":"hi"}"#),
            Ok(Event::Show {
                line: 7,
                text: n("hi")
            })
        );
        let ask = r#"{"line":8,"event":"ask","name":"i","question":"q?","answer":"a"}"#;
        assert_eq!(
            decode(ask),
            Ok(Event::Ask {
                line: 8,
                name: n("i"),
                question: n("q?"),
                answer: n("a")
            })
        );
        let file =
            r#"{"line":9,"event":"file","table":"t","file":"t.csv","action":"clear","rows":0}"#;
        assert_eq!(
            decode(file),
            Ok(Event::File {
                line: 9,
                table: n("t"),
                file: n("t.csv"),
                action: FileAction::Clear,
                rows: 0
            })
        );
        let fail = r#"{"line":2,"event":"fail","failure":4,"message":"no"}"#;
        assert_eq!(
            decode(fail),
            Ok(Event::Fail {
                line: 2,
                failure: 4,
                message: n("no")
            })
        );
    }

    #[test]
    fn a_bad_event_is_an_error_that_names_the_key() {
        let err = |text| decode(text).unwrap_err();
        assert_eq!(
            err(r#"{"line":1,"event":"jump"}"#),
            "unknown event \"jump\""
        );
        assert_eq!(err(r#"{"line":1,"event":"show"}"#), "missing key \"text\"");
        assert_eq!(
            err(r#"{"line":"1","event":"show","text":""}"#),
            "key \"line\" must be an integer"
        );
        assert_eq!(
            err(r#"{"line":1,"event":"loop","name":"p","turn":"1","value":{"number":"1"}}"#),
            "key \"turn\" must be an integer"
        );
        assert_eq!(
            err(r#"{"line":1,"event":"set","name":"p","to":{"number":"1"},"item":"x"}"#),
            "key \"item\" must be an integer"
        );
        assert_eq!(err(r#"{"event":"show","text":""}"#), "missing key \"line\"");
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

    #[test]
    fn more_bad_events_name_the_key() {
        let err = |text: &str| decode(text).unwrap_err();
        let line = |extra: &str| format!(r#"{{"line":1,"event":{extra}}}"#);
        assert_eq!(
            err(r#"{"line":4294967296,"event":"show","text":""}"#),
            "key \"line\" is out of range"
        );
        assert_eq!(
            err(r#"{"line":-1,"event":"show","text":""}"#),
            "key \"line\" is out of range"
        );
        let action = line(r#""file","table":"t","file":"f","action":"zap","rows":0"#);
        assert_eq!(err(&action), "key \"action\" must be load, save or clear");
        let result = line(r#""check","condition":"c","values":{},"result":"maybe""#);
        assert_eq!(err(&result), "key \"result\" must be yes or no");
        let missing = [
            (r#""set","to":{"number":"1"}"#, "name"),
            (r#""set","name":"a""#, "to"),
            (r#""check","values":{},"result":"yes""#, "condition"),
            (r#""check","condition":"c","result":"yes""#, "values"),
            (r#""check","condition":"c","values":{}"#, "result"),
            (r#""loop","name":"p","turn":1"#, "value"),
            (r#""loop","name":"p","value":{"number":"1"}"#, "turn"),
            (r#""enter","feature":"f""#, "inputs"),
            (r#""enter","inputs":{}"#, "feature"),
            (r#""file","table":"t","file":"f","action":"load""#, "rows"),
            (r#""file","table":"t","action":"load","rows":0"#, "file"),
            (r#""ask","name":"n","question":"q""#, "answer"),
            (r#""fail","failure":1"#, "message"),
        ];
        for (rest, key) in missing {
            assert_eq!(err(&line(rest)), format!("missing key \"{key}\""), "{rest}");
        }
    }
}
