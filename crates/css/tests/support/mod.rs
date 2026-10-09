//! Test-only helpers: a minimal JSON reader for the Chrome corpora (the crate has no dependencies), a checker
//! that records every mismatch with its case, and `Doc`, a small element tree that implements `Element`. The
//! corpus generators (tests/css/gen) write the trees and the states; the expected values come from Chrome.

#![allow(dead_code)]

use std::fmt::Debug;

use vn_css::matching::{Element, ElementState};

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
    /// Parses a corpus file. Corpora come from our generators, so malformed input panics.
    pub fn parse(src: &str) -> Json {
        let mut p = Parser { b: src.as_bytes(), i: 0 };
        let v = p.value();
        p.ws();
        assert_eq!(p.i, p.b.len(), "trailing data in corpus at byte {}", p.i);
        v
    }

    pub fn get(&self, key: &str) -> &Json {
        match self.opt(key) {
            Some(v) => v,
            None => panic!("corpus has no key {key:?}"),
        }
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
        let mut units: Vec<u16> = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => units.push(e as u16),
                        b'b' => units.push(0x08),
                        b'f' => units.push(0x0C),
                        b'n' => units.push(b'\n' as u16),
                        b'r' => units.push(b'\r' as u16),
                        b't' => units.push(b'\t' as u16),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex escape");
                            units.push(u16::from_str_radix(hex, 16).expect("hex escape"));
                            self.i += 4;
                        }
                        other => panic!("bad escape \\{}", other as char),
                    }
                }
                _ => {
                    // Raw UTF-8: collect the bytes of this code point and decode them below.
                    let start = self.i - 1;
                    let mut end = self.i;
                    while end < self.b.len() && (self.b[end] & 0xC0) == 0x80 {
                        end += 1;
                    }
                    let chunk = std::str::from_utf8(&self.b[start..end]).expect("utf-8 corpus string");
                    units.extend(chunk.encode_utf16());
                    self.i = end;
                }
            }
        }
        String::from_utf16(&units).expect("well-formed corpus string")
    }
}

/// Accumulates the comparisons of one corpus table. Every case is counted; mismatches are kept with their row.
pub struct Checker {
    table: &'static str,
    checked: usize,
    failures: Vec<String>,
}

impl Checker {
    pub fn new(table: &'static str) -> Checker {
        Checker { table, checked: 0, failures: Vec::new() }
    }

    pub fn check<T: PartialEq + Debug>(&mut self, row: usize, case: &str, actual: T, expected: T) {
        self.checked += 1;
        if actual != expected {
            self.failures.push(format!("{}[{row}] {case}: actual {actual:?}, expected {expected:?}", self.table));
        }
    }

    pub fn checked(&self) -> usize {
        self.checked
    }

    /// Ok with the number of cases when everything matched, else the first 20 mismatches.
    pub fn finish(self) -> Result<usize, String> {
        if self.failures.is_empty() {
            return Ok(self.checked);
        }
        let mut msg = format!("{}: {} of {} cases mismatch (first 20 shown)\n", self.table, self.failures.len(), self.checked);
        for f in self.failures.iter().take(20) {
            msg.push_str(f);
            msg.push('\n');
        }
        Err(msg)
    }
}

/// One element of a fixture tree. `index` is the pre-order element index (the same numbering the Chrome
/// generator uses for `document.querySelectorAll('*')`).
#[derive(Debug, Clone)]
pub struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub parent: Option<usize>,
    pub prev: Option<usize>,
    pub next: Option<usize>,
    pub first: Option<usize>,
    pub last: Option<usize>,
    pub state: ElementState,
    pub root: bool,
}

/// An element tree built from a corpus tree description.
#[derive(Debug, Clone)]
pub struct Doc {
    pub nodes: Vec<Node>,
}

/// A handle to one element of a `Doc`; implements the matcher's `Element` trait.
#[derive(Debug, Clone, Copy)]
pub struct El<'d> {
    pub doc: &'d Doc,
    pub idx: usize,
}

impl Doc {
    /// Builds the tree from `{"t":tag,"a":[[name,value],...],"s":{state...},"c":[children...]}`. Text nodes in the
    /// description are not elements and are skipped. `s` holds Chrome's state for the element.
    pub fn from_json(root: &Json) -> Doc {
        let mut doc = Doc { nodes: Vec::new() };
        doc.add(root, None, true);
        doc
    }

    fn add(&mut self, j: &Json, parent: Option<usize>, root: bool) -> usize {
        let idx = self.nodes.len();
        let state = parse_state(j.opt("s"));
        let attrs = match j.opt("a") {
            Some(a) => a.arr().iter().map(|p| (p.arr()[0].str().to_string(), p.arr()[1].str().to_string())).collect(),
            None => Vec::new(),
        };
        self.nodes.push(Node {
            tag: j.get("t").str().to_string(),
            attrs,
            parent,
            prev: None,
            next: None,
            first: None,
            last: None,
            state,
            root,
        });
        let mut prev: Option<usize> = None;
        let mut first = None;
        if let Some(children) = j.opt("c") {
            for c in children.arr() {
                if c.opt("t").is_none() {
                    continue; // text or comment node
                }
                let ci = self.add(c, Some(idx), false);
                if first.is_none() {
                    first = Some(ci);
                }
                self.nodes[ci].prev = prev;
                if let Some(p) = prev {
                    self.nodes[p].next = Some(ci);
                }
                prev = Some(ci);
            }
        }
        self.nodes[idx].first = first;
        self.nodes[idx].last = prev;
        idx
    }

    pub fn el(&self, idx: usize) -> El<'_> {
        El { doc: self, idx }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Element indices in document order that `pred` accepts.
    pub fn select(&self, mut pred: impl FnMut(El<'_>) -> bool) -> Vec<usize> {
        (0..self.nodes.len()).filter(|&i| pred(self.el(i))).collect()
    }
}

fn parse_state(s: Option<&Json>) -> ElementState {
    let flag = |k: &str| s.and_then(|s| s.opt(k)).is_some_and(|v| v.bool());
    ElementState {
        hover: flag("hover"),
        active: flag("active"),
        focus: flag("focus"),
        focus_visible: flag("focusVisible"),
        focus_within: flag("focusWithin"),
        disabled: flag("disabled"),
        enabled: flag("enabled"),
        checked: flag("checked"),
        empty: flag("empty"),
    }
}

impl<'d> El<'d> {
    fn node(&self) -> &'d Node {
        &self.doc.nodes[self.idx]
    }

    fn at(&self, idx: Option<usize>) -> Option<El<'d>> {
        idx.map(|i| El { doc: self.doc, idx: i })
    }
}

impl Element for El<'_> {
    fn local_name(&self) -> &str {
        &self.node().tag
    }

    fn namespace_is_html(&self) -> bool {
        true
    }

    fn id(&self) -> Option<&str> {
        self.attribute("id")
    }

    fn has_class(&self, class: &str) -> bool {
        self.attribute("class").is_some_and(|c| c.split_ascii_whitespace().any(|w| w == class))
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.node().attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn parent_element(&self) -> Option<Self> {
        self.at(self.node().parent)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.at(self.node().prev)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.at(self.node().next)
    }

    fn first_child_element(&self) -> Option<Self> {
        self.at(self.node().first)
    }

    fn last_child_element(&self) -> Option<Self> {
        self.at(self.node().last)
    }

    fn is_root(&self) -> bool {
        self.node().root
    }

    fn state(&self) -> ElementState {
        self.node().state
    }
}

/// Builds an element with explicit attributes and state, for hand-written matcher tests.
pub fn fake(tag: &str, attrs: &[(&str, &str)], state: ElementState) -> Doc {
    Doc {
        nodes: vec![Node {
            tag: tag.to_string(),
            attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            parent: None,
            prev: None,
            next: None,
            first: None,
            last: None,
            state,
            root: false,
        }],
    }
}
