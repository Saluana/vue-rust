//! Shared harness for the WP-RT-B tests. A realm gets the `vue` exports of vn-runtime (`exports::all`, rooted here so
//! the collector keeps them), and a JS array `log` that native test closures append to. Native closures get their state
//! through function data, never through Rust captures, so the collector sees every value they use.
#![allow(dead_code)]

use std::collections::HashMap;

use vn_runtime::exports::all;
use vn_value::{Call, CodeFn, JsStr, ObjRef, PropKey, Res, Rt, RootId, Value};

pub struct Env {
    pub rt: Rt,
    pub vue: HashMap<&'static str, Value>,
    pub log: ObjRef,
    _roots: Vec<RootId>,
}

impl Env {
    pub fn new() -> Env {
        let rt = Rt::new();
        let exports = all(&rt);
        let mut roots = Vec::new();
        let mut vue = HashMap::new();
        for (name, v) in exports {
            roots.push(rt.root(&v));
            vue.insert(name, v);
        }
        let log = rt.new_array(Vec::new());
        roots.push(rt.root(&Value::Object(log)));
        Env { rt, vue, log, _roots: roots }
    }

    /// One of the `vue` module's exports, by name.
    pub fn export(&self, name: &str) -> Value {
        self.vue.get(name).cloned().unwrap_or_else(|| panic!("vue export {name} (test bug)"))
    }

    pub fn native(&self, name: &str, code: CodeFn, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, name, 0, data))
    }

    pub fn s(&self, text: &str) -> Value {
        Value::String(JsStr::from_str(text))
    }

    pub fn log_value(&self) -> Value {
        Value::Object(self.log)
    }

    /// Appends `label` to the log: the body of the `logger` native.
    pub fn log_lines(&self) -> Vec<String> {
        let rt = &self.rt;
        let len = rt.to_number(&rt.get_v(&self.log_value(), &PropKey::from_str("length")).expect("log length")).expect("n") as u32;
        (0..len)
            .map(|i| {
                let v = rt.get_v(&self.log_value(), &PropKey::Index(i)).expect("log item");
                rt.to_string(&v).expect("log text").to_rust_string_lossy()
            })
            .collect()
    }
}

/// Appends the callee's second data value (a label) to the log array in the first data value.
pub fn logger(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], d[1].clone())?;
    Ok(Value::Undefined)
}

pub fn append(rt: &Rt, arr: &Value, v: Value) -> Res<()> {
    let len = rt.to_number(&rt.get_str(arr, "length")?)? as u32;
    rt.set_v_strict(arr, &PropKey::Index(len), v)
}

pub fn label(env: &Env, name: &str) -> Value {
    env.s(name)
}

/// A job whose body appends `label` to the log.
pub fn logging_job(env: &Env, label_text: &str) -> ObjRef {
    env.rt.new_native_function(logger, label_text, 0, vec![env.log_value(), env.s(label_text)])
}

pub fn text_of(rt: &Rt, v: &Value) -> String {
    rt.to_string(v).expect("text").to_rust_string_lossy()
}

/// A plain object with the given data properties.
pub fn object(rt: &Rt, pairs: Vec<(&str, Value)>) -> Value {
    let o = rt.new_object();
    for (k, v) in pairs {
        rt.define_data(o, PropKey::from_str(k), v, vn_value::Flags::ALL).expect("fresh object");
    }
    Value::Object(o)
}

/// Appends the text of every argument to the log (the body of `log_args`). Data: [log].
pub fn log_args(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    for a in c.args {
        let text = rt.to_string(a)?;
        append(rt, &d[0], Value::String(text))?;
    }
    Ok(Value::Undefined)
}

/// Appends `label`, then returns the data value at index 2. Data: [log, label, result].
pub fn log_and_return(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    append(rt, &d[0], d[1].clone())?;
    Ok(d[2].clone())
}

/// Throws a TypeError with the message given in the first data value.
pub fn throw_message(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    Err(rt.throw_type_error(&text_of(rt, &d[0])))
}
