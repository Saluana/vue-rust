//! Strict JSON reader for the replay harness: the builtin corpora and the cross-check fixtures. It returns the
//! writer's `Json` type. Strings are UTF-16 code units (so `\ud800` survives), and numbers keep their literal
//! text, which the decoder parses with `f64::from_str`. Malformed input is an error value, never a panic.
//! Rejected: trailing text, leading zeros, control characters inside strings, duplicate object keys, and nesting
//! deeper than `MAX_DEPTH`.

use crate::json::Json;

/// Deepest allowed nesting of arrays and objects.
pub const MAX_DEPTH: usize = 256;

/// Why the input was refused, and where (a byte offset).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

/// Parses one JSON value that spans the whole text (surrounding whitespace is allowed).
pub fn parse(text: &str) -> Result<Json, JsonError> {
    let mut p = Parser { text, pos: 0, depth: 0 };
    p.skip_ws();
    let value = p.value()?;
    p.skip_ws();
    if p.pos != text.len() {
        return Err(p.err("trailing text after the value"));
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    fn err(&self, message: &str) -> JsonError {
        JsonError { offset: self.pos, message: message.to_string() }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes()[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn enter(&mut self) -> Result<(), JsonError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting is too deep"));
        }
        Ok(())
    }

    fn object(&mut self) -> Result<Json, JsonError> {
        self.enter()?;
        self.pos += 1;
        let mut pairs: Vec<(Vec<u16>, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
        } else {
            loop {
                self.skip_ws();
                if self.peek() != Some(b'"') {
                    return Err(self.err("expected a string key"));
                }
                let key = self.string()?;
                if pairs.iter().any(|(k, _)| *k == key) {
                    return Err(self.err("duplicate object key"));
                }
                self.skip_ws();
                if self.peek() != Some(b':') {
                    return Err(self.err("expected ':'"));
                }
                self.pos += 1;
                self.skip_ws();
                let value = self.value()?;
                pairs.push((key, value));
                self.skip_ws();
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b'}') => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(self.err("expected ',' or '}'")),
                }
            }
        }
        self.depth -= 1;
        Ok(Json::Obj(pairs))
    }

    fn array(&mut self) -> Result<Json, JsonError> {
        self.enter()?;
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
        } else {
            loop {
                self.skip_ws();
                items.push(self.value()?);
                self.skip_ws();
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b']') => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(self.err("expected ',' or ']'")),
                }
            }
        }
        self.depth -= 1;
        Ok(Json::Arr(items))
    }

    /// A string literal, as UTF-16 code units. The opening quote is at `pos`.
    fn string(&mut self) -> Result<Vec<u16>, JsonError> {
        self.pos += 1;
        let mut out: Vec<u16> = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.pos += 1;
                    let unit = match e {
                        b'"' => 0x22,
                        b'\\' => 0x5c,
                        b'/' => 0x2f,
                        b'b' => 0x08,
                        b'f' => 0x0c,
                        b'n' => 0x0a,
                        b'r' => 0x0d,
                        b't' => 0x09,
                        b'u' => self.hex4()?,
                        _ => return Err(self.err("invalid escape")),
                    };
                    out.push(unit);
                }
                0x00..=0x1f => return Err(self.err("control character inside a string")),
                0x20..=0x7f => {
                    out.push(u16::from(b));
                    self.pos += 1;
                }
                _ => {
                    // Multi-byte UTF-8 (the input is a &str, so the sequence is valid and starts here).
                    let Some(c) = self.text[self.pos..].chars().next() else {
                        return Err(self.err("invalid UTF-8 boundary"));
                    };
                    let mut buf = [0u16; 2];
                    out.extend_from_slice(c.encode_utf16(&mut buf));
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, JsonError> {
        let mut v: u16 = 0;
        for _ in 0..4 {
            let Some(d) = self.peek() else {
                return Err(self.err("truncated \\u escape"));
            };
            let digit = match d {
                b'0'..=b'9' => d - b'0',
                b'a'..=b'f' => d - b'a' + 10,
                b'A'..=b'F' => d - b'A' + 10,
                _ => return Err(self.err("invalid hex digit in \\u escape")),
            };
            v = (v << 4) | u16::from(digit);
            self.pos += 1;
        }
        Ok(v)
    }

    /// A JSON number. Its literal text is kept, so the decoder can parse it exactly.
    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.err("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("digits expected after '.'"));
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("digits expected in the exponent"));
            }
            self.digits();
        }
        Ok(Json::Num(self.text[start..self.pos].to_string()))
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::json::Json;

    #[test]
    fn reads_nested_values_and_keeps_number_text() {
        let v = parse(r#" {"a":[1,-0.5,1e+21,true,null],"b":{}} "#).expect("valid");
        assert_eq!(v.field("a").and_then(Json::items).map(<[Json]>::len), Some(5));
        assert_eq!(v.field("a").and_then(|a| a.items()).and_then(|i| i[2].num_text()), Some("1e+21"));
    }

    #[test]
    fn rejects_malformed_input_with_an_offset() {
        assert!(parse("").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("01").is_err(), "leading zero");
        assert!(parse("1.").is_err());
        assert!(parse("\"a\u{1}\"").is_err(), "raw control character");
        assert!(parse(r#"{"a":1,"a":2}"#).is_err(), "duplicate key");
        assert!(parse("[] x").is_err(), "trailing text");
        assert!(parse(r#""\x""#).is_err(), "invalid escape");
    }

    #[test]
    fn decodes_escapes_and_lone_surrogates_to_code_units() {
        let v = parse(r#""a\"\\\b\f\n\r\t\u0000😀\ud83d!""#).expect("valid");
        assert_eq!(
            v.units().expect("string"),
            &[0x61, 0x22, 0x5c, 0x08, 0x0c, 0x0a, 0x0d, 0x09, 0x00, 0xd83d, 0xde00, 0xd83d, 0x21]
        );
        let raw = parse("\"\u{1F600}\u{e9}\"").expect("valid");
        assert_eq!(raw.units().expect("string"), &[0xd83d, 0xde00, 0xe9]);
    }

    #[test]
    fn refuses_nesting_past_the_limit() {
        let deep = format!("{}{}", "[".repeat(300), "]".repeat(300));
        assert!(parse(&deep).is_err());
        let ok = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(parse(&ok).is_ok());
    }
}
