//! A small JSON reader for event lines. Integers only (no fractions, exponents or leading
//! zeros; `-0` reads as 0); objects keep their key order, and a repeated key is an error.
//! It is lenient about whitespace between tokens: the four RFC 8259 characters (space, tab,
//! line feed, carriage return) are skipped, though the traced program writes none.

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
        depth: 0,
    };
    let value = r.value()?;
    r.skip_space();
    match r.peek() {
        None => Ok(value),
        Some(_) => Err(format!("text after the value at byte {}", r.at)),
    }
}

/// Arrays and objects nest at most this deep, so a hostile line cannot exhaust the stack.
const MAX_DEPTH: usize = 64;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
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
            Some(b'[') => self.nested(Self::array),
            Some(b'{') => self.nested(Self::object),
            Some(b'-' | b'0'..=b'9') => self.int(),
            _ => Err(format!("no value at byte {}", self.at)),
        }
    }

    fn nested(&mut self, read: fn(&mut Self) -> Result<Json, String>) -> Result<Json, String> {
        if self.depth == MAX_DEPTH {
            return Err("nested too deeply".to_string());
        }
        self.depth += 1;
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn int(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        let digits = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        if self.at - digits > 1 && self.bytes[digits] == b'0' {
            return Err(format!("leading zero at byte {start}"));
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
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(format!("repeated key \"{key}\""));
            }
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

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or("short \\u escape")?;
        let text = std::str::from_utf8(digits).map_err(|_| "bad \\u escape")?;
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("bad \\u escape".to_string());
        }
        self.at += 4;
        u32::from_str_radix(text, 16).map_err(|_| "bad \\u escape".to_string())
    }

    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        let code = match first {
            0xD800..=0xDBFF => {
                if !self.bytes[self.at..].starts_with(b"\\u") {
                    return Err("lone surrogate".to_string());
                }
                self.at += 2;
                let second = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&second) {
                    return Err("lone surrogate".to_string());
                }
                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err("lone surrogate".to_string()),
            _ => first,
        };
        char::from_u32(code).ok_or_else(|| "bad \\u escape".to_string())
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
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
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
        let empty = parse(r#"{"inputs":{},"xs":[],"ok":true,"no":false,"z":null}"#).unwrap();
        let Json::Object(pairs) = empty else { panic!() };
        assert_eq!(pairs[0].1, Json::Object(vec![]));
        assert_eq!(pairs[1].1, Json::Array(vec![]));
        assert_eq!(pairs[2].1, Json::Bool(true));
        assert_eq!(pairs[4].1, Json::Null);
        assert_eq!(
            parse(r#"{"list":[{"number":"1"},{"number":"2"}]}"#).unwrap(),
            Json::Object(vec![(
                "list".into(),
                Json::Array(vec![
                    Json::Object(vec![("number".into(), s("1"))]),
                    Json::Object(vec![("number".into(), s("2"))]),
                ])
            )])
        );
    }

    #[test]
    fn reads_every_escape() {
        assert_eq!(
            parse(r#""a\"b\\c\nd\/\t\r\b\f""#).unwrap(),
            s("a\"b\\c\nd/\t\r\u{8}\u{c}")
        );
        assert_eq!(parse(r#""é""#).unwrap(), s("é"));
        assert_eq!(parse(r#""😀""#).unwrap(), s("😀"));
        assert_eq!(parse("\"\u{fffd}\"").unwrap(), s("\u{fffd}"));
        assert_eq!(parse(r#""�""#).unwrap(), s("\u{fffd}"));
    }

    #[test]
    fn reads_integers() {
        assert_eq!(parse("-12").unwrap(), Json::Int(-12));
        assert_eq!(parse(" 0 ").unwrap(), Json::Int(0));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            r#"{"a":1} x"#,
            r#""\ud83d""#,
            r#""\ude00""#,
            r#""\ud83dx""#,
            "1.5",
            "1e3",
            r#""abc"#,
            r#"{"a":1"#,
            "[1,",
            r#""\q""#,
            "99999999999999999999",
            "",
            "tru",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        for bad in ["007", "-01", "-", "\"a\u{1}b\"", "\"a\nb\""] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let nested = |n: usize| "[".repeat(n) + &"]".repeat(n);
        assert!(parse(&nested(64)).is_ok());
        assert_eq!(parse(&nested(65)).unwrap_err(), "nested too deeply");
        assert_eq!(
            parse(r#"{"a":1,"b":2,"a":3}"#).unwrap_err(),
            "repeated key \"a\""
        );
    }

    #[test]
    fn reads_unicode_escapes_written_as_json() {
        assert_eq!(parse(r#""\u00e9""#).unwrap(), s("é"));
        assert_eq!(parse(r#""\u00E9""#).unwrap(), s("é"));
        assert_eq!(parse(r#""\ud83d\ude00""#).unwrap(), s("😀"));
        assert_eq!(parse(r#""\uD83D\uDE00""#).unwrap(), s("😀"));
        assert_eq!(parse(r#""\ufffd""#).unwrap(), s("\u{fffd}"));
    }

    #[test]
    fn numbers_follow_rfc_8259_for_zero() {
        assert_eq!(parse("0").unwrap(), Json::Int(0));
        assert_eq!(parse("-0").unwrap(), Json::Int(0));
        assert_eq!(parse("10").unwrap(), Json::Int(10));
    }

    #[test]
    fn skips_the_four_rfc_whitespace_characters() {
        let spaced = " \t\r\n{ \"a\" :\t[ 1 ,\n2 ]\r}\n";
        let want = Json::Object(vec![(
            "a".into(),
            Json::Array(vec![Json::Int(1), Json::Int(2)]),
        )]);
        assert_eq!(parse(spaced).unwrap(), want);
    }
}
