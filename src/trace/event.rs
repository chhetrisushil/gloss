//! Typed events: what each line of a trace holds, decoded from its JSON.

use super::json::Json;

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
}
