//! Shared support of the WP-RT-D tests: a JSON reader that keeps object key order, the loader of the Chrome corpora
//! (tests/ui/runtime-d/corpora), and the replayer that runs one recorded case through `DocHost` and the runtime's DOM
//! layer, then compares every recorded result. The replayer implements the op language written by
//! tests/ui/runtime-d/gen/page-run.mjs; the expectations are what headless Chrome recorded, never what the Rust code says.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vn_dom::{EventInit, EventKind};
use vn_runtime::dom_events::{node_of_handle, DomCtx};
use vn_runtime::dom_props::patch_prop;
use vn_runtime::host::{Host, HostNode, HostNs, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_value::{payload_ref, Call, Flags, Intrinsic, JsStr, NativeObject, PropKey, Res, Rt, Throw, Tracer, Value};

// ---- JSON (key order kept) ----

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

    pub fn as_arr(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    /// Compact JSON text (used in mismatch messages and for the snapshot-free comparisons).
    pub fn render(&self) -> String {
        match self {
            Json::Null => "null".to_string(),
            Json::Bool(b) => b.to_string(),
            Json::Num(n) => format!("{n}"),
            Json::Str(s) => format!("{s:?}"),
            Json::Arr(a) => format!("[{}]", a.iter().map(Json::render).collect::<Vec<_>>().join(",")),
            Json::Obj(f) => format!("{{{}}}", f.iter().map(|(k, v)| format!("{k:?}:{}", v.render())).collect::<Vec<_>>().join(",")),
        }
    }
}

pub fn parse_json(src: &str) -> Result<Json, String> {
    let mut p = Parser { src, i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != src.len() {
        return Err(format!("trailing characters at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    src: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn bytes(&self) -> &'a [u8] {
        self.src.as_bytes()
    }

    fn ws(&mut self) {
        while self.i < self.src.len() && matches!(self.bytes()[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let Some(&b) = self.bytes().get(self.i) else { return Err("unexpected end of JSON".into()) };
        match b {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Ok(Json::Str(self.string()?)),
            b't' | b'f' | b'n' => self.literal(),
            _ => self.number(),
        }
    }

    fn literal(&mut self) -> Result<Json, String> {
        for (text, v) in [("true", Json::Bool(true)), ("false", Json::Bool(false)), ("null", Json::Null)] {
            if self.src[self.i..].starts_with(text) {
                self.i += text.len();
                return Ok(v);
            }
        }
        Err(format!("bad literal at byte {}", self.i))
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while self.i < self.src.len() && matches!(self.bytes()[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
            self.i += 1;
        }
        self.src[start..self.i].parse::<f64>().map(Json::Num).map_err(|_| format!("bad number at byte {start}"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let s = self.src.get(self.i..self.i + 4).ok_or("short \\u escape")?;
        self.i += 4;
        u32::from_str_radix(s, 16).map_err(|_| "bad \\u escape".to_string())
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let Some(c) = self.src[self.i..].chars().next() else { return Err("unterminated string".into()) };
            self.i += c.len_utf8();
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.src[self.i..].chars().next().ok_or("bad escape")?;
                    self.i += e.len_utf8();
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) && self.src[self.i..].starts_with("\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            } else {
                                out.push(char::from_u32(hi).unwrap_or('\u{fffd}'));
                            }
                        }
                        other => return Err(format!("bad escape \\{other}")),
                    }
                }
                other => out.push(other),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.bytes().get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.bytes().get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected , or ] at byte {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut fields = Vec::new();
        self.ws();
        if self.bytes().get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.ws();
            if self.bytes().get(self.i) != Some(&b'"') {
                return Err(format!("expected key at byte {}", self.i));
            }
            let k = self.string()?;
            self.ws();
            if self.bytes().get(self.i) != Some(&b':') {
                return Err(format!("expected : at byte {}", self.i));
            }
            self.i += 1;
            let v = self.value()?;
            fields.push((k, v));
            self.ws();
            match self.bytes().get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                _ => return Err(format!("expected , or }} at byte {}", self.i)),
            }
        }
    }
}

// ---- corpora ----

/// The corpus file `name` under tests/ui/runtime-d/corpora: `[header, case, case, ...]`.
pub fn load_corpus(name: &str) -> (Json, Vec<Json>) {
    let path = format!("{}/../../tests/ui/runtime-d/corpora/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let all = parse_json(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let items = all.as_arr().expect("corpus is an array").clone();
    let mut it = items.into_iter();
    let header = it.next().expect("corpus header");
    (header, it.collect())
}

// ---- replay ----

/// The first recorded result that the replay does not reproduce.
#[derive(Debug)]
pub struct Mismatch {
    pub case: String,
    pub step: usize,
    pub op: String,
    pub expected: String,
    pub actual: String,
}

/// Per-case environment shared with the handler functions (they log, and their actions patch elements).
pub struct Env {
    pub ctx: DomCtx,
    pub host: DocHost,
    pub handler_specs: Json,
    pub exports: HashMap<&'static str, Value>,
    pub logs: RefCell<Vec<String>>,
    pub els: RefCell<HashMap<String, HostNode>>,
    pub handlers: RefCell<HashMap<String, Value>>,
    pub refs: RefCell<HashMap<String, Value>>,
}

impl Env {
    fn node(&self, id: &str) -> HostNode {
        *self.els.borrow().get(id).unwrap_or_else(|| panic!("no element {id}"))
    }

    fn export(&self, name: &str) -> Value {
        self.exports.get(name).cloned().unwrap_or_else(|| panic!("no export {name}"))
    }
}

#[derive(Clone)]
enum Action {
    Stop,
    StopImmediate,
    Prevent,
    Throw,
    Patch { el: String, key: String, prev: Json, next: Json, ns: Option<String> },
}

struct HandlerSpec {
    name: String,
    log: Vec<String>,
    then: Vec<Action>,
    env: Rc<Env>,
}

impl NativeObject for HandlerSpec {
    fn trace(&self, _t: &mut Tracer) {}
}

fn strings(j: Option<&Json>) -> Vec<String> {
    j.and_then(Json::as_arr).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn actions(j: Option<&Json>) -> Vec<Action> {
    let Some(items) = j.and_then(Json::as_arr) else { return Vec::new() };
    items
        .iter()
        .map(|a| match a {
            Json::Str(s) => match s.as_str() {
                "stopPropagation" => Action::Stop,
                "stopImmediatePropagation" => Action::StopImmediate,
                "preventDefault" => Action::Prevent,
                "throw" => Action::Throw,
                other => panic!("unknown action {other}"),
            },
            Json::Obj(_) => Action::Patch {
                el: a.get("el").and_then(Json::as_str).expect("patch el").to_string(),
                key: a.get("key").and_then(Json::as_str).expect("patch key").to_string(),
                prev: a.get("prev").cloned().unwrap_or(Json::Null),
                next: a.get("next").cloned().unwrap_or(Json::Null),
                ns: a.get("ns").and_then(Json::as_str).map(str::to_string),
            },
            other => panic!("bad action {other:?}"),
        })
        .collect()
}

fn ns_of(s: Option<&str>) -> HostNs {
    match s {
        Some("svg") => HostNs::Svg,
        Some("math") | Some("mathml") => HostNs::MathMl,
        _ => HostNs::Html,
    }
}

/// `String(n)` for the numbers the logs carry (integers print without a fraction).
fn js_number(n: f64) -> String {
    if n == 0.0 {
        "0".to_string()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn message_of(rt: &Rt, v: &Value) -> String {
    match rt.get_str(v, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => rt.to_string(v).map(|s| s.to_rust_string_lossy()).unwrap_or_default(),
    }
}

/// `Object.prototype.toString.call(v) === "[object Set]"`.
fn is_set(rt: &Rt, v: &Value) -> Res<bool> {
    if !matches!(v, Value::Object(_)) {
        return Ok(false);
    }
    let proto = Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype));
    let to_string = rt.get_str(&proto, "toString")?;
    let tag = rt.call(&to_string, v, &[])?;
    Ok(matches!(tag, Value::String(s) if s.to_rust_string_lossy() == "[object Set]"))
}

fn items_of(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    if rt.is_array(v) {
        let len = rt.to_number(&rt.get_str(v, "length")?)? as u32;
        let mut out = Vec::new();
        for i in 0..len {
            out.push(rt.get_v(v, &PropKey::Index(i))?);
        }
        return Ok(out);
    }
    let arr = vn_value::builtins::array::array_from_iterable(rt, v)?;
    items_of(rt, &Value::Object(arr))
}

/// The log encoding of a value (page-run.mjs `fmt`).
fn fmt_value(rt: &Rt, env: &Env, v: &Value) -> Res<String> {
    Ok(match v {
        Value::Null => "null".to_string(),
        Value::Undefined => "undef".to_string(),
        Value::String(s) => format!("s:{}", s.to_rust_string_lossy()),
        Value::Number(n) => format!("n:{}", js_number(*n)),
        Value::Bool(b) => format!("b:{b}"),
        Value::Object(_) if rt.is_array(v) => {
            let mut parts = Vec::new();
            for x in items_of(rt, v)? {
                parts.push(fmt_value(rt, env, &x)?);
            }
            format!("a[{}]", parts.join(","))
        }
        Value::Object(_) => match node_of_handle(rt, v) {
            Some(node) => format!("el:{}", env.host.clone().tag_name(node)),
            None if is_set(rt, v)? => {
                let mut parts = Vec::new();
                for x in items_of(rt, v)? {
                    parts.push(fmt_value(rt, env, &x)?);
                }
                format!("S{{{}}}", parts.join(","))
            }
            None => "o".to_string(),
        },
        _ => "o".to_string(),
    })
}

/// The log encoding of one field (page-run.mjs `fieldVal`); the field `$` is the value itself.
fn field_value(rt: &Rt, env: &Env, e: &Value, field: &str) -> Res<String> {
    if field == "$" {
        return fmt_value(rt, env, e);
    }
    if !rt.in_operator(&Value::String(JsStr::from_str(field)), e)? {
        return Ok("<absent>".to_string());
    }
    let v = rt.get_str(e, field)?;
    fmt_value(rt, env, &v)
}

fn call_method(rt: &Rt, e: &Value, name: &str) -> Res<()> {
    let f = rt.get_str(e, name)?;
    rt.call(&f, e, &[])?;
    Ok(())
}

fn handler_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let holder = data[0].as_object().expect("spec holder");
    let payload = rt.native_payload(holder).expect("spec payload");
    let spec = payload_ref::<HandlerSpec>(&*payload).expect("handler spec");
    let (name, log, then, env) = (spec.name.clone(), spec.log.clone(), spec.then.clone(), Rc::clone(&spec.env));
    drop(payload);
    let e = c.arg(0);
    let mut line = name;
    for f in &log {
        let v = field_value(rt, &env, &e, f)?;
        line.push('|');
        line.push_str(f);
        line.push('=');
        line.push_str(&v);
    }
    env.logs.borrow_mut().push(line);
    for a in &then {
        match a {
            Action::Throw => return Err(rt.throw_type_error("boom")),
            Action::Stop => call_method(rt, &e, "stopPropagation")?,
            Action::StopImmediate => call_method(rt, &e, "stopImmediatePropagation")?,
            Action::Prevent => call_method(rt, &e, "preventDefault")?,
            Action::Patch { el, key, prev, next, ns } => {
                let node = env.node(el);
                let p = decode(rt, &env, prev)?;
                let n = decode(rt, &env, next)?;
                patch_prop(&env.ctx, node, key, &p, &n, ns_of(ns.as_deref()))?;
            }
        }
    }
    Ok(Value::Undefined)
}

/// The handler function named `name` of the case (created once; rooted for the realm's lifetime).
fn handler(env: &Rc<Env>, name: &str) -> Value {
    if let Some(v) = env.handlers.borrow().get(name).cloned() {
        return v;
    }
    let spec_json = env.handler_specs.get(name).unwrap_or_else(|| panic!("no handler spec {name}"));
    let rt = env.ctx.rt();
    let spec = HandlerSpec {
        name: name.to_string(),
        log: strings(spec_json.get("log")),
        then: actions(spec_json.get("then")),
        env: Rc::clone(env),
    };
    let holder = rt.new_native_object(None, Rc::new(spec));
    let f = Value::Object(rt.new_native_function(handler_code, name, 1, vec![Value::Object(holder)]));
    rt.root(&f);
    env.handlers.borrow_mut().insert(name.to_string(), f.clone());
    f
}

/// Decodes a corpus value: `{"$":"u"}` undefined, `{"$":"fn","n"}` a handler, `{"$":"ref","id"}` a wrapper made by a
/// wrap op, `{"$":"set","items"}` a Set; arrays and other objects are JS ones.
fn decode(rt: &Rt, env: &Rc<Env>, j: &Json) -> Res<Value> {
    Ok(match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Num(n) => Value::Number(*n),
        Json::Str(s) => Value::String(JsStr::from_str(s)),
        Json::Arr(items) => {
            let mut out = Vec::with_capacity(items.len());
            for x in items {
                out.push(decode(rt, env, x)?);
            }
            Value::Object(rt.new_array(out))
        }
        Json::Obj(fields) => match j.get("$").and_then(Json::as_str) {
            Some("u") => Value::Undefined,
            Some("fn") => handler(env, j.get("n").and_then(Json::as_str).expect("handler name")),
            Some("ref") => {
                let id = j.get("id").and_then(Json::as_str).expect("ref id");
                env.refs.borrow().get(id).cloned().unwrap_or_else(|| panic!("no ref {id}"))
            }
            Some("set") => {
                let items = match j.get("items") {
                    Some(Json::Arr(a)) => a.clone(),
                    _ => Vec::new(),
                };
                let mut out = Vec::with_capacity(items.len());
                for x in &items {
                    out.push(decode(rt, env, x)?);
                }
                let global = Value::Object(rt.intrinsic(Intrinsic::Global));
                let ctor = rt.get_str(&global, "Set")?;
                rt.construct(&ctor, &[Value::Object(rt.new_array(out))], &ctor)?
            }
            _ => {
                let o = rt.new_object();
                for (k, v) in fields {
                    let dv = decode(rt, env, v)?;
                    rt.define_data(o, PropKey::from_str(k), dv, Flags::ALL)?;
                }
                Value::Object(o)
            }
        },
    })
}

fn str_field<'a>(step: &'a Json, key: &str) -> Result<&'a str, String> {
    step.get(key).and_then(Json::as_str).ok_or_else(|| format!("missing string field {key}"))
}

fn event_init(ev: &Json) -> EventInit {
    let kind = match ev.get("kind").and_then(Json::as_str) {
        Some("Mouse") => EventKind::Mouse,
        Some("Keyboard") => EventKind::Keyboard,
        Some("Input") => EventKind::Input,
        _ => EventKind::Event,
    };
    let ty = ev.get("type").and_then(Json::as_str).unwrap_or("");
    let mut init = EventInit::new(ty, kind);
    init.bubbles = matches!(ev.get("bubbles"), Some(Json::Bool(true)));
    init.cancelable = matches!(ev.get("cancelable"), Some(Json::Bool(true)));
    init.composed = matches!(ev.get("composed"), Some(Json::Bool(true)));
    let fields: &[(String, Json)] = match ev.get("init") {
        Some(Json::Obj(f)) => f,
        _ => &[],
    };
    for (k, v) in fields {
        let flag = matches!(v, Json::Bool(true));
        let num = match v {
            Json::Num(n) => *n,
            _ => 0.0,
        };
        match k.as_str() {
            "key" => init.key = v.as_str().unwrap_or("").to_string(),
            "code" => init.code = v.as_str().unwrap_or("").to_string(),
            "ctrlKey" => init.ctrl_key = flag,
            "shiftKey" => init.shift_key = flag,
            "altKey" => init.alt_key = flag,
            "metaKey" => init.meta_key = flag,
            "isComposing" => init.is_composing = flag,
            "button" => init.button = num as i32,
            "clientX" => init.client_x = num as i32,
            "clientY" => init.client_y = num as i32,
            "detail" => init.detail = num as i32,
            "data" => init.input_data = v.as_str().map(str::to_string),
            _ => {}
        }
    }
    init
}

fn message(rt: &Rt, t: Throw) -> String {
    message_of(rt, &t.0)
}

fn set_prop(env: &Env, node: HostNode, key: &str, value: HostValue) -> Result<(), String> {
    env.host.clone().set_property(node, key, value).map_err(|e| format!("{e:?}"))
}

/// Runs one op. `Some(json)` for the ops whose result the corpus records (snap, dispatch, log).
fn run_step(rt: &Rt, env: &Rc<Env>, step: &Json) -> Result<Option<Json>, String> {
    let op = str_field(step, "op")?;
    let node_of = |key: &str| -> Result<HostNode, String> {
        let id = str_field(step, key)?;
        env.els.borrow().get(id).copied().ok_or_else(|| format!("no element {id}"))
    };
    match op {
        "create" => {
            let id = str_field(step, "id")?.to_string();
            let tag = str_field(step, "tag")?;
            let ns = ns_of(step.get("ns").and_then(Json::as_str));
            let node = env.host.clone().create_element(ns, tag, None).map_err(|e| format!("{e:?}"))?;
            env.els.borrow_mut().insert(id, node);
            Ok(None)
        }
        "mount" => {
            let child = node_of("id")?;
            let parent = match step.get("parent").and_then(Json::as_str) {
                Some(p) => env.node(p),
                None => env.node("root"),
            };
            env.host.clone().insert(child, parent, None).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "patch" => {
            let node = node_of("el")?;
            let key = str_field(step, "key")?;
            let prev = decode(rt, env, step.get("prev").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let next = decode(rt, env, step.get("next").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let ns = ns_of(step.get("ns").and_then(Json::as_str));
            patch_prop(&env.ctx, node, key, &prev, &next, ns).map_err(|t| message(rt, t))?;
            Ok(None)
        }
        "snap" => {
            let node = node_of("el")?;
            let s = env.host.clone().snapshot(node).map_err(|e| format!("{e:?}"))?;
            Ok(Some(Json::Str(s)))
        }
        "dispatch" => {
            let node = node_of("el")?;
            let init = event_init(step.get("event").ok_or("dispatch without event")?);
            let prevented = env.host.clone().dispatch_event(node, init).map_err(|e| format!("{e:?}"))?;
            Ok(Some(Json::Bool(prevented)))
        }
        "log" => {
            let log: Vec<Json> = env.logs.borrow_mut().drain(..).map(Json::Str).collect();
            let errors: Vec<Json> = env.ctx.take_errors().into_iter().map(|v| Json::Str(message_of(rt, &v))).collect();
            Ok(Some(Json::Obj(vec![("log".to_string(), Json::Arr(log)), ("errors".to_string(), Json::Arr(errors))])))
        }
        "time" => {
            let ms = match step.get("ms") {
                Some(Json::Num(n)) => *n,
                _ => return Err("time without ms".into()),
            };
            env.host.set_now(ms);
            Ok(None)
        }
        "setProp" => {
            let node = node_of("el")?;
            let key = str_field(step, "key")?;
            let value = match step.get("value") {
                Some(Json::Str(s)) => HostValue::Str(s.clone()),
                Some(Json::Bool(b)) => HostValue::Bool(*b),
                Some(Json::Num(n)) => HostValue::Number(*n),
                _ => HostValue::Null,
            };
            set_prop(env, node, key, value)?;
            Ok(None)
        }
        "remove" => {
            let node = node_of("el")?;
            env.host.clone().remove(node).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "directive" => {
            let node = node_of("el")?;
            let name = str_field(step, "name")?;
            let hook_name = str_field(step, "hook")?;
            let dir = env.export(name);
            let hook = rt.get_str(&dir, hook_name).map_err(|t| message(rt, t))?;
            let binding = decode(rt, env, step.get("binding").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let vnode = decode(rt, env, step.get("vnode").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let prev = match step.get("prevVNode") {
                Some(p) => decode(rt, env, p).map_err(|t| message(rt, t))?,
                None => Value::Undefined,
            };
            let el = env.ctx.handle(node);
            rt.call(&hook, &Value::Undefined, &[el, binding, vnode, prev]).map_err(|t| message(rt, t))?;
            Ok(None)
        }
        "wrap" => {
            let id = str_field(step, "id")?.to_string();
            let kind = str_field(step, "kind")?;
            let wrapper = env.export(kind);
            let f = decode(rt, env, step.get("fn").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let mods = decode(rt, env, step.get("mods").unwrap_or(&Json::Null)).map_err(|t| message(rt, t))?;
            let made = rt.call(&wrapper, &Value::Undefined, &[f, mods]).map_err(|t| message(rt, t))?;
            env.refs.borrow_mut().insert(id, made);
            Ok(None)
        }
        "type" => {
            let node = node_of("el")?;
            let value = str_field(step, "value")?.to_string();
            set_prop(env, node, "value", HostValue::Str(value.clone()))?;
            let mut init = EventInit::new("input", EventKind::Input);
            init.bubbles = true;
            init.composed = true;
            init.input_data = Some(value);
            env.host.clone().dispatch_event(node, init).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "compose" => {
            let node = node_of("el")?;
            let ty = if str_field(step, "phase")? == "start" { "compositionstart" } else { "compositionend" };
            let mut init = EventInit::new(ty, EventKind::Event);
            init.bubbles = true;
            init.composed = true;
            env.host.clone().dispatch_event(node, init).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "click" => {
            let node = node_of("el")?;
            env.host.clone().click(node).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "fire" => {
            let node = node_of("el")?;
            let mut init = EventInit::new(str_field(step, "type")?, EventKind::Event);
            init.bubbles = true;
            init.composed = true;
            env.host.clone().dispatch_event(node, init).map_err(|e| format!("{e:?}"))?;
            Ok(None)
        }
        "select" => {
            let node = node_of("el")?;
            let indices: Vec<usize> = match step.get("indices") {
                Some(Json::Arr(a)) => a.iter().filter_map(|x| if let Json::Num(n) = x { Some(*n as usize) } else { None }).collect(),
                _ => Vec::new(),
            };
            let mut host = env.host.clone();
            let multiple = matches!(host.get_property(node, "multiple"), Ok(HostValue::Bool(true)));
            if multiple {
                for (i, opt) in host.option_nodes(node).into_iter().enumerate() {
                    set_prop(env, opt, "selected", HostValue::Bool(indices.contains(&i)))?;
                }
            } else {
                let idx = indices.first().map(|i| *i as f64).unwrap_or(-1.0);
                set_prop(env, node, "selectedIndex", HostValue::Number(idx))?;
            }
            Ok(None)
        }
        other => Err(format!("harness error: unknown op {other}")),
    }
}

/// Replays one recorded case in a fresh realm and DOM. Returns the number of recorded results compared, or the first
/// result the replay does not reproduce. A step that threw in Chrome (`{"$error": ...}`) must throw here too; the
/// messages are not compared (V8 and the runtime word their TypeErrors differently).
pub fn replay(case: &Json) -> Result<usize, Mismatch> {
    let name = case.get("name").and_then(Json::as_str).unwrap_or("?").to_string();
    let rt = Rt::new();
    let mut host = DocHost::new();
    let body = host.install_body().expect("body");
    let ctx = DomCtx::new(&rt, &host);
    let root = host.create_element(HostNs::Html, "div", None).expect("root");
    host.insert(root, body, None).expect("mount root");
    let exports: HashMap<&'static str, Value> = vn_runtime::exports::all(&rt).into_iter().collect();
    let env = Rc::new(Env {
        ctx,
        host: host.clone(),
        handler_specs: case.get("handlers").cloned().unwrap_or(Json::Obj(Vec::new())),
        exports,
        logs: RefCell::new(Vec::new()),
        els: RefCell::new(HashMap::from([("root".to_string(), root)])),
        handlers: RefCell::new(HashMap::new()),
        refs: RefCell::new(HashMap::new()),
    });
    let steps = case.get("steps").and_then(Json::as_arr).expect("steps");
    let results = case.get("results").and_then(Json::as_arr).expect("results");
    assert_eq!(steps.len(), results.len(), "{name}: steps and results differ in length");
    let mut checked = 0;
    for (i, (step, expected)) in steps.iter().zip(results.iter()).enumerate() {
        let op = step.get("op").and_then(Json::as_str).unwrap_or("").to_string();
        let mismatch = |actual: String| Mismatch { case: name.clone(), step: i, op: op.clone(), expected: expected.render(), actual };
        // the page awaits a microtask before each op: queued nextTick callbacks run first
        env.ctx.flush_next_tick();
        let chrome_threw = expected.get("$error").is_some();
        match run_step(&rt, &env, step) {
            Ok(None) => {
                if chrome_threw {
                    return Err(mismatch("no error was thrown".to_string()));
                }
            }
            Ok(Some(actual)) => {
                if chrome_threw {
                    return Err(mismatch(actual.render()));
                }
                if &actual != expected {
                    return Err(mismatch(actual.render()));
                }
                checked += 1;
            }
            Err(msg) => {
                if chrome_threw {
                    checked += 1;
                    continue;
                }
                return Err(mismatch(format!("error: {msg}")));
            }
        }
    }
    Ok(checked)
}
