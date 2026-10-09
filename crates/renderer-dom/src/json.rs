//! Minimal JSON reader and writer for the self-test protocol (feature `selftest`, and unit tests). No dependencies.
//! Non-finite numbers are written as the strings "NaN", "Infinity" and "-Infinity", which the self-test reads back as numbers.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    #[cfg(feature = "selftest")]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

const MAX_DEPTH: u32 = 200;

pub fn parse(text: &str) -> Result<Json, String> {
    let mut p = Parser { s: text.as_bytes(), i: 0, depth: 0 };
    let value = p.value()?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(format!("trailing characters at byte {}", p.i));
    }
    Ok(value)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.nested(Self::object),
            Some(b'[') => self.nested(Self::array),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(c) => Err(format!("unexpected '{}' at byte {}", c as char, self.i)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn nested(&mut self, f: fn(&mut Self) -> Result<Json, String>) -> Result<Json, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("nesting too deep".to_string());
        }
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn word(&mut self, text: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(text.as_bytes()) {
            self.i += text.len();
            Ok(v)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(format!("expected a key at byte {}", self.i));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected ':' at byte {}", self.i));
            }
            self.i += 1;
            fields.push((key, self.value()?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.s.get(self.i..self.i + 4).ok_or("truncated \\u escape")?;
        let text = std::str::from_utf8(digits).map_err(|_| "bad \\u escape")?;
        let v = u32::from_str_radix(text, 16).map_err(|_| format!("bad \\u escape '{text}'"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else { return Err("unterminated string".to_string()) };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else { return Err("unterminated escape".to_string()) };
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return Err("lone high surrogate".to_string());
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err("bad low surrogate".to_string());
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            char::from_u32(code).ok_or("lone surrogate in \\u escape")?
                        }
                        other => return Err(format!("bad escape '\\{}'", other as char)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| "invalid UTF-8".to_string())
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while matches!(self.peek(), Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')) {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "bad number")?;
        text.parse::<f64>().map(Json::Num).map_err(|_| format!("bad number '{text}' at byte {start}"))
    }
}

pub fn write(v: &Json) -> String {
    let mut out = String::new();
    emit(v, &mut out);
    out
}

fn emit(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) if n.is_nan() => out.push_str("\"NaN\""),
        Json::Num(n) if n.is_infinite() => out.push_str(if *n > 0.0 { "\"Infinity\"" } else { "\"-Infinity\"" }),
        Json::Num(n) => out.push_str(&format!("{n}")),
        Json::Str(s) => emit_str(s, out),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit(item, out);
            }
            out.push(']');
        }
        Json::Obj(fields) => {
            out.push('{');
            for (i, (k, item)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit_str(k, out);
                out.push(':');
                emit(item, out);
            }
            out.push('}');
        }
    }
}

fn emit_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_values() {
        let v = parse(r#" [ {"a": 1.5e2, "b": [true, false, null]}, "x" ] "#).unwrap();
        let first = &v.as_arr().unwrap()[0];
        assert_eq!(first.get("a"), Some(&Json::Num(150.0)));
        assert_eq!(first.get("b"), Some(&Json::Arr(vec![Json::Bool(true), Json::Bool(false), Json::Null])));
        assert_eq!(v.as_arr().unwrap()[1].as_str(), Some("x"));
    }

    #[test]
    fn decodes_escapes_and_surrogate_pairs() {
        assert_eq!(parse(r#""a\u0000b\n\"é""#).unwrap(), Json::Str("a\0b\n\"é".to_string()));
        assert_eq!(parse(r#""\ud83d\ude00""#).unwrap(), Json::Str("😀".to_string()));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in ["", "[1,]", "{\"a\" 1}", "\"open", "[1] x", "\"\\ud83d\"", "\"\\q\"", "tru"] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
        let too_deep = "[".repeat(300) + &"]".repeat(300);
        assert!(parse(&too_deep).unwrap_err().contains("too deep"));
    }

    #[test]
    fn writes_escaped_strings_and_non_finite_numbers() {
        let v = Json::Arr(vec![Json::Str("q\"\\\u{1}😀".to_string()), Json::Num(f64::NAN), Json::Num(-f64::INFINITY), Json::Num(12.0)]);
        assert_eq!(write(&v), "[\"q\\\"\\\\\\u0001😀\",\"NaN\",\"-Infinity\",12]");
        assert_eq!(parse(&write(&v)).unwrap().as_arr().unwrap()[3], Json::Num(12.0));
    }
}
