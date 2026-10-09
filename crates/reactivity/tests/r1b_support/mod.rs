//! Shared helpers of the WP-R1b tests. The oracle is tests/vue/rust/reactive/expected.json, written by gen-expected.mjs from
//! the real @vue/reactivity 3.5.43 production build. `log_line!` is the Rust form of the snippets' `log(...)`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

use vn_value::{Flags, Intrinsic, JsStr, ObjRef, PropKey, Res, Rt, Value, WellKnown};

use super::common::Env;

const ORACLE: &str = include_str!("../../../../tests/vue/rust/reactive/expected.json");

/// `log(...)` of a snippet: the parts (already `String(v)`) joined by single spaces.
#[macro_export]
macro_rules! log_line {
    ($e:expr, $($part:expr),+ $(,)?) => {
        $e.say(&[$(::std::string::ToString::to_string(&$part)),+])
    };
}

fn cases() -> &'static HashMap<String, Vec<String>> {
    static CASES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    CASES.get_or_init(|| parse_cases(ORACLE))
}

/// The oracle log of snippet `id`.
pub fn expected(id: &str) -> Vec<String> {
    cases().get(id).unwrap_or_else(|| panic!("no oracle case {id} (test bug)")).clone()
}

/// Every snippet id of the R1b oracle, sorted.
pub fn oracle_ids() -> Vec<String> {
    let mut ids: Vec<String> = cases().keys().cloned().collect();
    ids.sort();
    ids
}

/// Asserts that the log of `e` equals the oracle log of `id`.
pub fn check(e: &Env, id: &str) {
    let got = e.take();
    let want = expected(id);
    assert_eq!(got, want, "case {id}\n  vue:  {want:?}\n  rust: {got:?}");
}

/// `String(v)` of a value, as `log` prints it (the kernel's ToString, so arrays and proxies print their elements).
pub fn sv(e: &Env, v: &Value) -> String {
    e.rt.to_string(v).expect("String(v) (test bug)").to_rust_string_lossy()
}

/// `String(boolean)`.
pub fn bo(x: bool) -> String {
    x.to_string()
}

pub fn num(x: f64) -> Value {
    Value::Number(x)
}

pub fn st(s: &str) -> Value {
    Value::String(JsStr::from_str(s))
}

pub fn undef() -> Value {
    Value::Undefined
}

/// A plain object with the given data properties.
pub fn object(e: &Env, pairs: &[(&str, Value)]) -> Value {
    let o = e.rt.new_object();
    for (k, v) in pairs {
        e.rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).expect("object builder (test bug)");
    }
    Value::Object(o)
}

/// A global binding such as `Object` or `Promise`.
pub fn global(e: &Env, name: &str) -> Value {
    e.get(&Value::Object(e.rt.intrinsic(Intrinsic::Global)), name)
}

/// `String(exports[name](v))` for a predicate export such as `isReactive`.
pub fn flag(e: &Env, export: &str, v: &Value) -> String {
    bo(e.invoke(export, &[v.clone()]).to_boolean())
}

/// `Object.keys(v).join(sep)`.
pub fn keys_joined(e: &Env, v: &Value, sep: &str) -> String {
    let keys = method(e, &global(e, "Object"), "keys", &[v.clone()]);
    sv(e, &method(e, &keys, "join", &[st(sep)]))
}

pub fn array(e: &Env, items: Vec<Value>) -> Value {
    Value::Object(e.rt.new_array(items))
}

/// `Reflect.apply`-style call of a method: `obj[name](...args)`.
pub fn method(e: &Env, obj: &Value, name: &str, args: &[Value]) -> Value {
    let f = e.get(obj, name);
    e.call(&f, obj, args)
}

/// A `Map` with the given entries (`new Map(entries)`).
pub fn new_map(e: &Env, entries: &[(Value, Value)]) -> Value {
    let pairs: Vec<Value> = entries.iter().map(|(k, v)| array(e, vec![k.clone(), v.clone()])).collect();
    let list = array(e, pairs);
    let ctor = global(e, "Map");
    e.rt.construct(&ctor, &[list], &ctor).expect("new Map (test bug)")
}

/// A `Set` with the given items (`new Set(items)`).
pub fn new_set(e: &Env, items: &[Value]) -> Value {
    let list = array(e, items.to_vec());
    let ctor = global(e, "Set");
    e.rt.construct(&ctor, &[list], &ctor).expect("new Set (test bug)")
}

/// The values an iterable yields, as `[...iterable]` does (`Symbol.iterator`, then `next` until done), inside a closure.
pub fn iterate_rt(rt: &Rt, iterable: &Value) -> Res<Vec<Value>> {
    let f = rt.get_v(iterable, &PropKey::Sym(rt.well_known(WellKnown::Iterator)))?;
    let iter = rt.call(&f, iterable, &[])?;
    let next = rt.get_str(&iter, "next")?;
    let mut out = Vec::new();
    loop {
        let r = rt.call(&next, &iter, &[])?;
        if rt.get_str(&r, "done")?.to_boolean() {
            break;
        }
        out.push(rt.get_str(&r, "value")?);
    }
    Ok(out)
}

/// The values an iterable yields (see `iterate_rt`), from the test body.
pub fn iterate(e: &Env, iterable: &Value) -> Vec<Value> {
    iterate_rt(&e.rt, iterable).expect("iteration (test bug)")
}

/// A JS predicate `(x) => x[field] === want` (a number `want`).
pub fn field_is(e: &Env, field: &str, want: f64) -> Value {
    let field = field.to_string();
    func(e, "", 1, vec![], move |rt, _this, args, _d| {
        let v = rt.get_str(&args[0], &field)?;
        Ok(Value::Bool(matches!(v, Value::Number(n) if n == want)))
    })
}

/// A JS predicate `(x) => x[field] % modulus === want`.
pub fn field_mod_is(e: &Env, field: &str, modulus: f64, want: f64) -> Value {
    let field = field.to_string();
    func(e, "", 1, vec![], move |rt, _this, args, _d| {
        let v = rt.get_str(&args[0], &field)?;
        Ok(Value::Bool(matches!(v, Value::Number(n) if n % modulus == want)))
    })
}

/// `Object.prototype.toString.call(v)`.
pub fn tag_of(e: &Env, v: &Value) -> String {
    let ts = e.get(&Value::Object(e.rt.intrinsic(Intrinsic::ObjectPrototype)), "toString");
    sv(e, &e.call(&ts, v, &[]))
}

/// Runs `body` as an effect (`effect(body)`); the body receives the data values (`d`) and returns `Ok`.
pub fn run_effect<F>(e: &Env, data: Vec<Value>, body: F) -> Value
where
    F: Fn(&Rt, &[Value]) -> Res<Value> + 'static,
{
    let f = e.func("effect body", 0, data, move |rt, _this, _args, d| body(rt, d));
    e.invoke("effect", &[f])
}

/// A JS function running `body` with `this` and the call arguments (data `d` are the closure's own values).
pub fn func<F>(e: &Env, name: &str, len: u32, data: Vec<Value>, body: F) -> Value
where
    F: Fn(&Rt, &Value, &[Value], &[Value]) -> Res<Value> + 'static,
{
    e.func(name, len, data, body)
}

pub fn object_ref(v: &Value) -> ObjRef {
    v.as_object().expect("object value (test bug)")
}

// ----- the oracle file: a small reader for the JSON that gen-expected.mjs writes -----

enum J {
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
    Other,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> J {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut fields = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    let k = self.string();
                    self.ws();
                    assert_eq!(self.b[self.i], b':', "expected.json: ':' expected");
                    self.i += 1;
                    let v = self.value();
                    fields.push((k, v));
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                J::Obj(fields)
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    items.push(self.value());
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                J::Arr(items)
            }
            b'"' => J::Str(self.string()),
            _ => {
                while self.i < self.b.len() && !matches!(self.b[self.i], b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t') {
                    self.i += 1;
                }
                J::Other
            }
        }
    }

    fn string(&mut self) -> String {
        assert_eq!(self.b[self.i], b'"', "expected.json: string expected");
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
                            let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex escape");
                            self.i += 4;
                            let cp = u32::from_str_radix(hex, 16).expect("hex escape");
                            let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => panic!("expected.json: bad escape (test bug)"),
                    }
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).expect("expected.json: utf-8")
    }
}

fn parse_cases(src: &str) -> HashMap<String, Vec<String>> {
    let mut r = Reader { b: src.as_bytes(), i: 0 };
    let J::Obj(root) = r.value() else {
        panic!("expected.json: object expected (test bug)");
    };
    let cases = root.into_iter().find(|(k, _)| k == "cases").expect("expected.json: cases (test bug)").1;
    let J::Obj(cases) = cases else {
        panic!("expected.json: cases object expected (test bug)");
    };
    cases
        .into_iter()
        .map(|(id, lines)| {
            let J::Arr(lines) = lines else {
                panic!("expected.json: log array expected (test bug)");
            };
            let lines = lines
                .into_iter()
                .map(|l| match l {
                    J::Str(s) => s,
                    _ => panic!("expected.json: string expected (test bug)"),
                })
                .collect();
            (id, lines)
        })
        .collect()
}
