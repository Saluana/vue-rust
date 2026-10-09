//! A small JSON reader for the corpora (std only, like the crate). Numbers are f64; objects keep their key order.
//! Corpora come from the generators in tests/ui/style/gen, so malformed input panics.

#![allow(dead_code)]

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
    pub fn parse(src: &str) -> Json {
        let mut p = Parser { b: src.as_bytes(), i: 0 };
        let v = p.value();
        p.ws();
        assert_eq!(p.i, p.b.len(), "trailing data in corpus at byte {}", p.i);
        v
    }

    pub fn get(&self, key: &str) -> &Json {
        self.opt(key).unwrap_or_else(|| panic!("corpus has no key {key:?}"))
    }

    pub fn opt(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            other => panic!("expected an object, found {other:?}"),
        }
    }

    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(v) => v,
            other => panic!("expected an array, found {other:?}"),
        }
    }

    pub fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            other => panic!("expected a string, found {other:?}"),
        }
    }

    pub fn num(&self) -> f64 {
        match self {
            Json::Num(n) => *n,
            other => panic!("expected a number, found {other:?}"),
        }
    }

    pub fn bool(&self) -> bool {
        match self {
            Json::Bool(b) => *b,
            other => panic!("expected a boolean, found {other:?}"),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Json::Str(self.string()),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(_) => self.number(),
            None => panic!("unexpected end of corpus"),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Json {
        assert!(self.b[self.i..].starts_with(word.as_bytes()), "bad literal at byte {}", self.i);
        self.i += word.len();
        v
    }

    fn object(&mut self) -> Json {
        self.i += 1;
        let mut kv = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Json::Obj(kv);
        }
        loop {
            self.ws();
            let k = self.string();
            self.ws();
            assert_eq!(self.b.get(self.i), Some(&b':'), "expected ':' at byte {}", self.i);
            self.i += 1;
            let v = self.value();
            kv.push((k, v));
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Json::Obj(kv);
                }
                _ => panic!("expected ',' or '}}' at byte {}", self.i),
            }
        }
    }

    fn array(&mut self) -> Json {
        self.i += 1;
        let mut v = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Json::Arr(v);
        }
        loop {
            v.push(self.value());
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Json::Arr(v);
                }
                _ => panic!("expected ',' or ']' at byte {}", self.i),
            }
        }
    }

    fn hex4(&mut self) -> u32 {
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex escape");
        self.i += 4;
        u32::from_str_radix(s, 16).expect("hex escape digits")
    }

    fn string(&mut self) -> String {
        assert_eq!(self.b.get(self.i), Some(&b'"'), "expected a string at byte {}", self.i);
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut cp = self.hex4();
                            if (0xd800..0xdc00).contains(&cp) {
                                assert!(self.b[self.i..].starts_with(b"\\u"), "lone surrogate");
                                self.i += 2;
                                let lo = self.hex4();
                                cp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
                            }
                            let ch = char::from_u32(cp).expect("valid code point");
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        other => panic!("bad escape \\{}", other as char),
                    }
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).expect("corpus strings are UTF-8")
    }

    fn number(&mut self) -> Json {
        let start = self.i;
        while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).expect("number text");
        Json::Num(s.parse().unwrap_or_else(|_| panic!("bad number {s:?} at byte {start}")))
    }
}
