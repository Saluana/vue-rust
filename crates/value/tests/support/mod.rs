//! Test-only helpers for the corpus tests. The crate itself has no dependencies, so this file holds a
//! minimal JSON reader, a checker that records every mismatch with its case, and bit-exact comparison
//! for doubles. Corpus encoding is described in tests/language/gen/gen-corpora.mjs.

#![allow(dead_code)]

use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Parses a corpus file. The corpora are produced by our generator, so malformed input panics.
    pub fn parse(src: &str) -> Json {
        let mut p = Parser { b: src.as_bytes(), i: 0 };
        let v = p.value();
        p.ws();
        assert_eq!(p.i, p.b.len(), "trailing data in corpus at byte {}", p.i);
        v
    }

    pub fn get(&self, key: &str) -> &Json {
        match self {
            Json::Obj(kv) => match kv.iter().find(|(k, _)| k == key) {
                Some((_, v)) => v,
                None => panic!("corpus has no key {key:?}"),
            },
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

    pub fn int(&self) -> i64 {
        let n = self.num();
        assert!(n.fract() == 0.0 && n.abs() < 9.0e15, "expected an integer, found {n}");
        n as i64
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

    /// A JavaScript string: an array of UTF-16 code units.
    pub fn units(&self) -> Vec<u16> {
        self.arr()
            .iter()
            .map(|v| {
                let n = v.int();
                assert!((0..=0xFFFF).contains(&n), "code unit out of range: {n}");
                n as u16
            })
            .collect()
    }

    /// A JavaScript number: 16 lowercase hex digits of its IEEE-754 bits.
    pub fn bits(&self) -> u64 {
        let h = self.str();
        assert_eq!(h.len(), 16, "bit pattern must have 16 hex digits: {h:?}");
        u64::from_str_radix(h, 16).unwrap_or_else(|_| panic!("bad hex in corpus: {h:?}"))
    }

    pub fn f64(&self) -> f64 {
        f64::from_bits(self.bits())
    }

    /// Name of the thrown JavaScript error, for `{"throw":"RangeError"}`; `None` for a normal result.
    pub fn thrown(&self) -> Option<&str> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == "throw").map(|(_, v)| v.str()),
            _ => None,
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) {
        self.ws();
        assert_eq!(self.b.get(self.i).copied(), Some(c), "expected {:?} at byte {}", c as char, self.i);
        self.i += 1;
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut kv = Vec::new();
                self.ws();
                if self.b[self.i] == b'}' {
                    self.i += 1;
                    return Json::Obj(kv);
                }
                loop {
                    self.ws();
                    let k = self.string();
                    self.expect(b':');
                    let v = self.value();
                    kv.push((k, v));
                    self.ws();
                    match self.b[self.i] {
                        b',' => self.i += 1,
                        b'}' => {
                            self.i += 1;
                            return Json::Obj(kv);
                        }
                        c => panic!("bad object separator {:?} at byte {}", c as char, self.i),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b[self.i] == b']' {
                    self.i += 1;
                    return Json::Arr(items);
                }
                loop {
                    items.push(self.value());
                    self.ws();
                    match self.b[self.i] {
                        b',' => self.i += 1,
                        b']' => {
                            self.i += 1;
                            return Json::Arr(items);
                        }
                        c => panic!("bad array separator {:?} at byte {}", c as char, self.i),
                    }
                }
            }
            b'"' => Json::Str(self.string()),
            b't' => {
                self.literal("true");
                Json::Bool(true)
            }
            b'f' => {
                self.literal("false");
                Json::Bool(false)
            }
            b'n' => {
                self.literal("null");
                Json::Null
            }
            _ => Json::Num(self.number()),
        }
    }

    fn literal(&mut self, word: &str) {
        assert!(self.b[self.i..].starts_with(word.as_bytes()), "bad literal at byte {}", self.i);
        self.i += word.len();
    }

    fn number(&mut self) -> f64 {
        let start = self.i;
        while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).expect("ascii number");
        text.parse().unwrap_or_else(|_| panic!("bad number {text:?} at byte {start}"))
    }

    fn string(&mut self) -> String {
        assert_eq!(self.b[self.i], b'"', "expected a string at byte {}", self.i);
        self.i += 1;
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => bytes.push(e),
                        b'b' => bytes.push(0x08),
                        b'f' => bytes.push(0x0C),
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex escape");
                            let code = u32::from_str_radix(hex, 16).expect("hex escape");
                            self.i += 4;
                            let ch = char::from_u32(code).expect("corpus escapes are not surrogates");
                            let mut buf = [0u8; 4];
                            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        other => panic!("bad escape \\{}", other as char),
                    }
                }
                _ => bytes.push(c),
            }
        }
        String::from_utf8(bytes).expect("utf-8 corpus string")
    }
}

/// Accumulates the comparisons of one corpus table. Every case is counted; each mismatch is kept with
/// its row number, the case, and both values.
pub struct Checker {
    table: &'static str,
    checked: usize,
    failures: Vec<String>,
}

impl Checker {
    pub fn new(table: &'static str) -> Checker {
        Checker { table, checked: 0, failures: Vec::new() }
    }

    /// Exact equality (strings, booleans, integers, code unit arrays).
    pub fn check<T: PartialEq + Debug>(&mut self, row: usize, case: &str, actual: T, expected: T) {
        self.checked += 1;
        if actual != expected {
            self.fail(row, case, format!("actual {actual:?}, expected {expected:?}"));
        }
    }

    /// A double against V8's bit pattern: bit-exact (so -0 and +0 differ), except that any NaN matches
    /// any NaN because JavaScript cannot observe NaN payloads.
    pub fn check_bits(&mut self, row: usize, case: &str, actual: f64, expected: u64) {
        self.checked += 1;
        let e = f64::from_bits(expected);
        let same = (actual.is_nan() && e.is_nan()) || actual.to_bits() == expected;
        if !same {
            self.fail(
                row,
                case,
                format!(
                    "actual {actual} ({:016x}), expected {e} ({expected:016x})",
                    actual.to_bits()
                ),
            );
        }
    }

    fn fail(&mut self, row: usize, case: &str, detail: String) {
        self.failures.push(format!("{}[{row}] {case}: {detail}", self.table));
    }

    pub fn checked(&self) -> usize {
        self.checked
    }

    /// Ok with the number of cases when everything matched, else the first 20 mismatches.
    pub fn finish(self) -> Result<usize, String> {
        if self.failures.is_empty() {
            return Ok(self.checked);
        }
        let mut msg = format!(
            "{}: {} of {} cases mismatch (first 20 shown)\n",
            self.table,
            self.failures.len(),
            self.checked
        );
        for f in self.failures.iter().take(20) {
            msg.push_str(f);
            msg.push('\n');
        }
        Err(msg)
    }
}

/// Rows of a corpus table as arrays.
pub fn rows<'a>(doc: &'a Json, table: &str) -> &'a [Json] {
    doc.get(table).arr()
}
