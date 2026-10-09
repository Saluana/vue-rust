//! Shared harness for the WP-R1a tests. A realm gets the `vue` exports (`vn_reactivity::exports`) and the internal
//! helpers. Native closures are registered in a table and receive their ObjRefs through the function's data, never
//! through Rust captures, so the collector sees every live value. `expected(id)` reads the oracle log of a snippet
//! (tests/vue/rust/expected.json, written by the real @vue/reactivity 3.5.43 production build).
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vn_reactivity::{exports, internal_exports};
use vn_value::{payload_ref, Call, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

pub type Body = Rc<dyn Fn(&Rt, &Value, &[Value], &[Value]) -> Res<Value>>;

#[derive(Default)]
pub struct Table {
    fns: RefCell<Vec<Body>>,
}

impl NativeObject for Table {
    fn trace(&self, _t: &mut Tracer) {}
}

/// The code of every closure made by `Env::func`: it finds the body by index and passes the data after the index.
fn dispatch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let idx = match data[0] {
        Value::Number(n) => n as usize,
        _ => panic!("closure index (test bug)"),
    };
    let obj = rt.builtin("test-closures").expect("test table registered");
    let payload = rt.native_payload(obj).expect("test table payload");
    let body = payload_ref::<Table>(&*payload).expect("test table").fns.borrow()[idx].clone();
    body(rt, c.this, c.args, &data[1..])
}

pub struct Env {
    pub rt: Rt,
    pub log: Rc<RefCell<Vec<String>>>,
    api: HashMap<&'static str, Value>,
    table: Rc<Table>,
}

impl Env {
    pub fn new() -> Env {
        let rt = Rt::new();
        let table: Rc<Table> = Rc::new(Table::default());
        let payload: Rc<dyn NativeObject> = table.clone();
        let obj = rt.new_native_object(None, payload);
        rt.register_builtin("test-closures", obj);
        let mut api = HashMap::new();
        for (name, v) in exports(&rt).into_iter().chain(internal_exports(&rt)) {
            api.insert(name, v);
        }
        Env { rt, log: Rc::new(RefCell::new(Vec::new())), api, table }
    }

    /// A JS function value that runs `body`; `data` is kept in the function object (GC-visible).
    pub fn func<F>(&self, name: &str, len: u32, data: Vec<Value>, body: F) -> Value
    where
        F: Fn(&Rt, &Value, &[Value], &[Value]) -> Res<Value> + 'static,
    {
        make_fn(&self.rt, &self.table, name, len, data, body)
    }

    /// The closure table, for closures that create closures (a maker returning a per-run function).
    pub fn table(&self) -> Rc<Table> {
        self.table.clone()
    }

    pub fn api(&self, name: &str) -> Value {
        self.api.get(name).unwrap_or_else(|| panic!("no export named {name} (test bug)")).clone()
    }

    /// Calls an export with an undefined receiver; a throw is a test failure.
    pub fn invoke(&self, name: &str, args: &[Value]) -> Value {
        match self.try_invoke(name, args) {
            Ok(v) => v,
            Err(t) => panic!("{name} threw: {}", self.show(&t.0)),
        }
    }

    pub fn try_invoke(&self, name: &str, args: &[Value]) -> Res<Value> {
        let f = self.api(name);
        self.rt.call(&f, &Value::Undefined, args)
    }

    pub fn get(&self, o: &Value, k: &str) -> Value {
        self.rt.get_str(o, k).expect("property read threw")
    }

    /// Strict assignment `o[k] = v` (module code is strict, so a refused write throws).
    pub fn set(&self, o: &Value, k: &str, v: Value) {
        self.rt.set_v_strict(o, &PropKey::from_str(k), v).expect("strict assignment threw");
    }

    pub fn try_set(&self, o: &Value, k: &str, v: Value) -> Res<()> {
        self.rt.set_v_strict(o, &PropKey::from_str(k), v)
    }

    pub fn call(&self, f: &Value, this: &Value, args: &[Value]) -> Value {
        match self.rt.call(f, this, args) {
            Ok(v) => v,
            Err(t) => panic!("call threw: {}", self.show(&t.0)),
        }
    }

    /// `String(v)` for the primitives the snippets log.
    pub fn show(&self, v: &Value) -> String {
        match v {
            Value::Object(_) => "[object Object]".to_string(),
            _ => self.rt.to_string(v).expect("to_string of a primitive").to_rust_string_lossy(),
        }
    }

    /// One logged line: the arguments joined by single spaces, as `log(...args)` in the snippets.
    pub fn say(&self, parts: &[String]) {
        self.log.borrow_mut().push(parts.join(" "));
    }

    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    pub fn text(s: &str) -> Value {
        Value::String(JsStr::from_str(s))
    }

    pub fn obj(o: ObjRef) -> Value {
        Value::Object(o)
    }
}

/// A JS function value running `body`, registered in `table` (see `Env::func`).
pub fn make_fn<F>(rt: &Rt, table: &Rc<Table>, name: &str, len: u32, data: Vec<Value>, body: F) -> Value
where
    F: Fn(&Rt, &Value, &[Value], &[Value]) -> Res<Value> + 'static,
{
    let idx = {
        let mut fns = table.fns.borrow_mut();
        fns.push(Rc::new(body));
        fns.len() - 1
    };
    let mut all = vec![Value::Number(idx as f64)];
    all.extend(data);
    Value::Object(rt.new_native_function(dispatch, name, len, all))
}

/// Snippet outcomes: what the oracle logged for `id`.
pub fn expected(id: &str) -> Vec<String> {
    let mut cases = parse_cases(include_str!("../../../../tests/vue/rust/expected.json"));
    cases.remove(id).unwrap_or_else(|| panic!("no oracle case {id} (test bug)"))
}

/// Every snippet id in the oracle file (used by the coverage audit).
pub fn oracle_ids() -> Vec<String> {
    let mut ids: Vec<String> = parse_cases(include_str!("../../../../tests/vue/rust/expected.json")).into_keys().collect();
    ids.sort();
    ids
}

/// Compares a log with the oracle; the error text shows both sides.
pub fn compare(id: &str, got: &[String]) -> Result<(), String> {
    let want = expected(id);
    if got == want.as_slice() {
        Ok(())
    } else {
        Err(format!("case {id}\n  vue:  {want:?}\n  rust: {got:?}"))
    }
}

/// Asserts that the log of `env` matches the oracle for `id`.
pub fn check(env: &Env, id: &str) {
    let got = env.take();
    if let Err(msg) = compare(id, &got) {
        panic!("{msg}");
    }
}

// ----- a minimal JSON reader for expected.json (objects, arrays, strings) -----

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
        while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> J {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut out = Vec::new();
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
                    out.push((k, v));
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                J::Obj(out)
            }
            b'[' => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    out.push(self.value());
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
                J::Arr(out)
            }
            b'"' => J::Str(self.string()),
            _ => {
                while self.i < self.b.len() && !matches!(self.b[self.i], b',' | b'}' | b']') {
                    self.i += 1;
                }
                J::Other
            }
        }
    }

    fn string(&mut self) -> String {
        assert_eq!(self.b[self.i], b'"', "expected.json: string expected");
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
                        b'n' => units.push(10),
                        b't' => units.push(9),
                        b'r' => units.push(13),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex");
                            units.push(u16::from_str_radix(hex, 16).expect("hex digits"));
                            self.i += 4;
                        }
                        other => units.push(other as u16),
                    }
                }
                _ => {
                    let start = self.i - 1;
                    let len = match c {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    self.i = start + len;
                    let s = std::str::from_utf8(&self.b[start..start + len]).expect("utf-8");
                    units.extend(s.encode_utf16());
                }
            }
        }
        String::from_utf16(&units).expect("expected.json strings are well-formed")
    }
}

fn parse_cases(src: &str) -> HashMap<String, Vec<String>> {
    let mut r = Reader { b: src.as_bytes(), i: 0 };
    let J::Obj(top) = r.value() else { panic!("expected.json: top-level object expected") };
    let (_, cases) = top.into_iter().find(|(k, _)| k == "cases").expect("expected.json: cases");
    let J::Obj(entries) = cases else { panic!("expected.json: cases object expected") };
    entries
        .into_iter()
        .map(|(k, v)| {
            let J::Arr(items) = v else { panic!("expected.json: log array expected") };
            let lines = items
                .into_iter()
                .map(|x| match x {
                    J::Str(s) => s,
                    _ => panic!("expected.json: string entries expected"),
                })
                .collect();
            (k, lines)
        })
        .collect()
}

/// `String(v)` of a value the snippets log (primitives; objects print as in a log line of Vue's snippets).
pub fn js(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Object(_) => "[object Object]".to_string(),
        _ => rt.to_string(v).expect("to_string of a primitive").to_rust_string_lossy(),
    }
}

/// Holder object: a GC-visible slot for a value a closure must reach later (e.g. a runner created after the closure).
pub fn holder(rt: &Rt) -> Value {
    Value::Object(rt.new_object())
}

pub fn put(rt: &Rt, h: &Value, k: &str, v: Value) {
    rt.set_v_strict(h, &vn_value::PropKey::from_str(k), v).expect("holder write");
}

pub fn take_from(rt: &Rt, h: &Value, k: &str) -> Value {
    rt.get_str(h, k).expect("holder read")
}
