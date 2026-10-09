//! Helpers shared by the testkit integration tests.
#![allow(dead_code)] // every test crate uses a different subset

use std::cell::RefCell;
use std::rc::Rc;

use vn_testkit::json::Json;
use vn_testkit::{install_host, Recorder, Timers};
use vn_value::{Call, Flags, FnInfo, FnKind, Intrinsic, JsStr, ObjRef, PropKey, Res, Rt, Value};

/// Describes the first byte at which `expected` and `actual` differ, with context. `None` when they are equal.
pub fn first_difference(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let (a, b) = (expected.as_bytes(), actual.as_bytes());
    let at = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let window = |s: &[u8]| {
        let centre = at.min(s.len());
        let start = centre.saturating_sub(60);
        let end = (centre + 60).min(s.len());
        String::from_utf8_lossy(&s[start..end]).into_owned()
    };
    Some(format!(
        "first difference at byte {at} (lengths {} vs {}):\n  expected: ...{}...\n  actual:   ...{}...",
        a.len(),
        b.len(),
        window(a),
        window(b)
    ))
}

/// A fresh realm with the scenario host installed, plus the recorder and the clock.
pub struct Realm {
    pub rt: Rt,
    pub rec: Rc<RefCell<Recorder>>,
    pub timers: Timers,
    pub trace: Value,
}

impl Realm {
    pub fn new() -> Realm {
        let rt = Rt::new();
        let rec = Rc::new(RefCell::new(Recorder::new()));
        let timers = install_host(&rt, &rec);
        let trace = rt.get_str(&global(&rt), "trace").expect("trace is installed");
        Realm { rt, rec, timers, trace }
    }

    pub fn global(&self) -> Value {
        global(&self.rt)
    }

    /// `trace(label, ...values)` as scenario code calls it.
    pub fn t(&self, label: &str, values: &[Value]) -> Res<()> {
        let mut args = vec![Value::str(label)];
        args.extend_from_slice(values);
        self.call_trace(&args)
    }

    /// `trace(...args)` with arbitrary arguments (for the label check).
    pub fn call_trace(&self, args: &[Value]) -> Res<()> {
        self.rt.call(&self.trace, &Value::Undefined, args)?;
        Ok(())
    }

    /// An ordinary object with the given own enumerable data properties, in order.
    pub fn obj(&self, pairs: &[(&str, Value)]) -> ObjRef {
        let o = self.rt.new_object();
        for (k, v) in pairs {
            self.rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).expect("define data property");
        }
        o
    }

    /// A user-style function object with the given name and length.
    pub fn func(&self, name: &str, length: u32, kind: FnKind) -> ObjRef {
        self.rt.new_function(noop, FnInfo::new(name, length, kind), &[])
    }

    /// `new Kind(...args)` for a builtin constructor.
    pub fn construct(&self, kind: Intrinsic, args: &[Value]) -> Res<Value> {
        let ctor = Value::Object(self.rt.intrinsic(kind));
        self.rt.construct(&ctor, args, &ctor)
    }

    /// The `values` of everything recorded so far, as the entries array text.
    pub fn entries_text(&self) -> String {
        Json::Arr(self.rec.borrow().entries().to_vec()).to_json()
    }
}

impl Default for Realm {
    fn default() -> Realm {
        Realm::new()
    }
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

pub fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

pub fn key(name: &str) -> PropKey {
    PropKey::from_str(name)
}

pub fn ov(o: ObjRef) -> Value {
    Value::Object(o)
}

pub fn num(x: f64) -> Value {
    Value::Number(x)
}

pub fn s(text: &str) -> Value {
    Value::str(text)
}

/// A string from UTF-16 code units (lone surrogates allowed).
pub fn units(u: &[u16]) -> Value {
    Value::String(JsStr::from_units(u))
}
