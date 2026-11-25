//! A small JSON reader for event lines. Integers only; objects keep their key order.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Reads one JSON value; anything after it is an error.
pub fn parse(line: &str) -> Result<Json, String> {
    let mut r = Reader {
        bytes: line.as_bytes(),
        at: 0,
    };
    let value = r.value()?;
    r.skip_space();
    match r.peek() {
        None => Ok(value),
        Some(_) => Err(format!("text after the value at byte {}", r.at)),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", byte as char, self.at))
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_space();
        match self.peek() {
            Some(b'n') => self.word("null", Json::Null),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.int(),
            _ => Err(format!("no value at byte {}", self.at)),
        }
    }

    fn int(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(format!("not an integer at byte {start}"));
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
        text.parse()
            .map(Json::Int)
            .map_err(|_| format!("not an integer at byte {start}"))
    }

    fn array(&mut self) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.at)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut pairs = Vec::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            self.skip_space();
            self.eat(b':')?;
            pairs.push((key, self.value()?));
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(pairs));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.at)),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err("unclosed string".to_string());
            };
            self.at += 1;
            match b {
                b'"' => return String::from_utf8(out).map_err(|_| "bad UTF-8".to_string()),
                b'\\' => {
                    let esc = self.peek().ok_or("unclosed string")?;
                    self.at += 1;
                    let ch = match esc {
                        b'"' => '"',
                        b'\\' => '\\',
                        _ => return Err(format!("bad escape at byte {}", self.at - 1)),
                    };
                    out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                }
                0..=0x1f => return Err(format!("raw control character at byte {}", self.at - 1)),
                _ => out.push(b),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Json {
        Json::Str(text.to_string())
    }

    #[test]
    fn reads_an_event_line_and_keeps_key_order() {
        let line =
            r#"{"line":3,"event":"set","name":"total","from":{"number":"0"},"to":{"number":"-5"}}"#;
        let Json::Object(pairs) = parse(line).unwrap() else {
            panic!()
        };
        let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["line", "event", "name", "from", "to"]);
        assert_eq!(pairs[0].1, Json::Int(3));
    }

    #[test]
    fn reads_quote_and_backslash_escapes() {
        assert_eq!(parse(r#""a\"b\\c""#).unwrap(), s("a\"b\\c"));
    }

    #[test]
    fn reads_integers() {
        assert_eq!(parse("-12").unwrap(), Json::Int(-12));
    }
}
