//! Canonical trace encoder (docs/dev/trace-format.md v1.1). It mirrors tooling/oracle/trace.mjs: ordinals for
//! objects and symbols in first-encounter order (functions take none), a per-call set of expanded objects
//! (repeats are `ref`s), properties in ordinary own-key order, and the oracle's key order, so both engines
//! print byte-identical documents. Accessors are never invoked. `name`/`message` of errors and `name`/`length`
//! of functions are read with [[Get]], as the oracle reads them.
//!
//! Map and Set are encoded from their collection payloads (WP-B4, `map_entries`, `set_values`); iterator objects
//! are plain objects. Promise objects are opaque: `{"$":"obj","cls":"Promise","proto":"other","props":[]}` (trace-format
//! v1). Other native host payloads (Date, RegExp, ...) are rejected with `TypeError("untraceable value")` before any
//! ordinal is assigned. Exotic objects are traced through their hooks like
//! ordinary objects, because the kernel's public API does not tell them apart (open question in the report).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use vn_value::builtins::promise::promise_state;
use vn_value::{number, Intrinsic, JsStr, ObjRef, Prop, PropKey, Res, Rt, Sym, Throw, Value};

use crate::json::Json;

/// Message of the `TypeError` thrown for a value the format cannot express.
pub const UNTRACEABLE: &str = "untraceable value";

/// Class of an encoded object, named as the oracle's `classOf` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Object,
    Array,
    Error,
    Map,
    Set,
    Promise,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Object => "Object",
            Class::Array => "Array",
            Class::Error => "Error",
            Class::Map => "Map",
            Class::Set => "Set",
            Class::Promise => "Promise",
        }
    }
}

/// Trace state of one run: the global ordinals, the objects expanded by the current `trace` call, and the
/// recorded entries. It holds no `Value`, so the collector has nothing to see in it.
#[derive(Default)]
pub struct Recorder {
    ids: HashMap<ObjRef, u32>,
    sym_ids: HashMap<Sym, u32>,
    seen: HashSet<ObjRef>,
    entries: Vec<Json>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder::default()
    }

    /// The recorded `{"label","values"}` entries, in call order.
    pub fn entries(&self) -> &[Json] {
        &self.entries
    }

    /// Ordinal of an object: 1, 2, 3 ... in order of first encounter (the map only grows).
    fn id_of(&mut self, o: ObjRef) -> u32 {
        let next = self.ids.len() as u32 + 1;
        *self.ids.entry(o).or_insert(next)
    }

    fn sym_id(&mut self, s: Sym) -> u32 {
        let next = self.sym_ids.len() as u32 + 1;
        *self.sym_ids.entry(s).or_insert(next)
    }
}

/// One `trace(label, ...values)` call. Each value is encoded with a fresh expansion set; the entry is appended
/// only when all values encode (a failed call records nothing, but the ordinals it assigned remain, as in the
/// oracle). Re-entrant: a getter may trace while this call runs, because no borrow is held across user code.
pub fn trace_call(rt: &Rt, rec: &RefCell<Recorder>, label: &JsStr, values: &[Value]) -> Res<()> {
    let outer = std::mem::take(&mut rec.borrow_mut().seen);
    let encoded: Res<Vec<Json>> = values.iter().map(|v| encode(rt, rec, v)).collect();
    rec.borrow_mut().seen = outer;
    let values = encoded?;
    rec.borrow_mut().entries.push(Json::obj([
        ("label", Json::Str(label.units().to_vec())),
        ("values", Json::Arr(values)),
    ]));
    Ok(())
}

/// A document-level encoding (`error`, `unhandledRejections`): fresh expansion set, shared ordinals.
pub fn encode_standalone(rt: &Rt, rec: &RefCell<Recorder>, v: &Value) -> Res<Json> {
    let outer = std::mem::take(&mut rec.borrow_mut().seen);
    let out = encode(rt, rec, v);
    rec.borrow_mut().seen = outer;
    out
}

/// One encoding pass over a list (the oracle's `encodeAll`): a single expansion set for the whole list, so
/// aliasing across the list is visible, and the ordinals of `rec`.
pub fn encode_pass(rt: &Rt, rec: &RefCell<Recorder>, values: &[Value]) -> Res<Vec<Json>> {
    let outer = std::mem::take(&mut rec.borrow_mut().seen);
    let out: Res<Vec<Json>> = values.iter().map(|v| encode(rt, rec, v)).collect();
    rec.borrow_mut().seen = outer;
    out
}

/// `encode(v)` of trace-format.md for one value, in the current expansion set.
pub fn encode(rt: &Rt, rec: &RefCell<Recorder>, v: &Value) -> Res<Json> {
    match v {
        Value::Undefined => Ok(Json::obj([("$", Json::text("undef"))])),
        Value::Null => Ok(Json::Null),
        Value::Bool(b) => Ok(Json::Bool(*b)),
        Value::Number(n) => Ok(number_json(*n)),
        Value::String(s) => Ok(Json::Str(s.units().to_vec())),
        Value::Symbol(s) => {
            let id = rec.borrow_mut().sym_id(*s);
            let desc = match rt.symbol_description(*s) {
                Some(d) => Json::Str(d.units().to_vec()),
                None => Json::Null,
            };
            Ok(Json::obj([("$", Json::text("sym")), ("id", Json::int(u64::from(id))), ("desc", desc)]))
        }
        Value::Object(o) if rt.is_callable(v) => encode_fn(rt, *o),
        Value::Object(o) => encode_object(rt, rec, *o),
        Value::Uninit | Value::Hole => Err(untraceable(rt)),
    }
}

fn untraceable(rt: &Rt) -> Throw {
    rt.throw_type_error(UNTRACEABLE)
}

/// Numbers: `-0`, `NaN` and the infinities have tagged forms; every other value is the text of `String(n)`.
pub fn number_json(n: f64) -> Json {
    if n == 0.0 && n.is_sign_negative() {
        return num_tag("-0");
    }
    if n.is_nan() {
        return num_tag("NaN");
    }
    if n.is_infinite() {
        return num_tag(if n > 0.0 { "Infinity" } else { "-Infinity" });
    }
    Json::Num(number::number_to_string(n))
}

fn num_tag(v: &str) -> Json {
    Json::obj([("$", Json::text("num")), ("v", Json::text(v))])
}

/// `{"$":"fn","name","length"}`: no ordinal. The oracle reads `v.name` and `v.length` with [[Get]]. A
/// non-string name or a non-number length has no faithful encoding, so it is rejected rather than guessed.
fn encode_fn(rt: &Rt, f: ObjRef) -> Res<Json> {
    let fv = Value::Object(f);
    let name = match rt.get_str(&fv, "name")? {
        Value::String(s) => s,
        _ => return Err(untraceable(rt)),
    };
    // JSON.stringify writes -0 as 0 (number_to_string does too) and non-finite numbers as null.
    let length = match rt.get_str(&fv, "length")? {
        Value::Number(n) if n.is_finite() => Json::Num(number::number_to_string(n)),
        Value::Number(_) => Json::Null,
        _ => return Err(untraceable(rt)),
    };
    Ok(Json::obj([("$", Json::text("fn")), ("name", Json::Str(name.units().to_vec())), ("length", length)]))
}

/// Objects, arrays and errors. The header keys come first, then `props`, then the class-specific keys, which is
/// the oracle's key order: `$ id cls proto [integrity] props [elems len | name message]`.
fn encode_object(rt: &Rt, rec: &RefCell<Recorder>, o: ObjRef) -> Res<Json> {
    // classOf runs before idOf, so a rejected value consumes no ordinal (as in the oracle).
    let class = class_of(rt, o)?;
    let id = u64::from(rec.borrow_mut().id_of(o));
    if !rec.borrow_mut().seen.insert(o) {
        return Ok(Json::obj([("$", Json::text("ref")), ("id", Json::int(id))]));
    }
    let mut head: Vec<(&str, Json)> = vec![
        ("$", Json::text("obj")),
        ("id", Json::int(id)),
        ("cls", Json::text(class.name())),
        ("proto", Json::text(proto_text(rt, o))),
    ];
    if let Some(integrity) = integrity_of(rt, o)? {
        head.push(("integrity", Json::text(integrity)));
    }
    if class == Class::Promise {
        // trace-format v1: a promise is opaque (its state is not observable here), so its own properties are not traced.
        head.push(("props", Json::Arr(Vec::new())));
        return Ok(Json::obj(head));
    }

    // Class-specific data is read before the own-key enumeration, as in the oracle.
    let mut tail: Vec<(&str, Json)> = Vec::new();
    match class {
        Class::Array => {
            let len = array_length(rt, o)?;
            let mut elems = Vec::with_capacity(len);
            let mut i = 0usize;
            // The oracle re-reads `o.length` on every iteration. Dense arrays are below 2^22 elements, so the
            // index fits in u32.
            while i < array_length(rt, o)? {
                match rt.get_own_property(o, &PropKey::Index(i as u32))? {
                    None => elems.push(Json::obj([("$", Json::text("hole"))])),
                    Some(p) => elems.push(prop_json(rt, rec, p)?),
                }
                i += 1;
            }
            tail.push(("elems", Json::Arr(elems)));
            tail.push(("len", number_json(len as f64)));
        }
        Class::Error => {
            let name = js_string_of(rt, &rt.get_str(&Value::Object(o), "name")?)?;
            let message = js_string_of(rt, &rt.get_str(&Value::Object(o), "message")?)?;
            tail.push(("name", Json::Str(name.units().to_vec())));
            tail.push(("message", Json::Str(message.units().to_vec())));
        }
        Class::Map => {
            let mut out = Vec::new();
            for (k, v) in vn_value::builtins::map_set::map_entries(rt, o).unwrap_or_default() {
                out.push(Json::Arr(vec![encode(rt, rec, &k)?, encode(rt, rec, &v)?]));
            }
            tail.push(("entries", Json::Arr(out)));
        }
        Class::Set => {
            let mut out = Vec::new();
            for v in vn_value::builtins::map_set::set_values(rt, o).unwrap_or_default() {
                out.push(encode(rt, rec, &v)?);
            }
            tail.push(("values", Json::Arr(out)));
        }
        Class::Object | Class::Promise => {}
    }

    let mut props = Vec::new();
    for key in rt.own_keys(o)? {
        match &key {
            PropKey::Sym(_) => continue,                            // symbol keys are not traced in v1
            PropKey::Index(_) if class == Class::Array => continue, // array indices are elems
            _ => {}
        }
        // A key listed by own_keys but gone now (a getter removed it while tracing) is an invalid scenario.
        let Some(p) = rt.get_own_property(o, &key)? else {
            return Err(untraceable(rt));
        };
        if !p.flags().enumerable {
            continue;
        }
        if class == Class::Error && is_key(&key, "stack") {
            continue; // stack is never traced
        }
        props.push(Json::Arr(vec![Json::Str(key_units(&key)), prop_json(rt, rec, p)?]));
    }

    let mut pairs = head;
    pairs.push(("props", Json::Arr(props)));
    pairs.extend(tail);
    Ok(Json::obj(pairs))
}

/// The encoding of an own property's value. An accessor is a marker: its getter is never called.
fn prop_json(rt: &Rt, rec: &RefCell<Recorder>, p: Prop) -> Res<Json> {
    match p {
        Prop::Accessor { get, set, .. } => Ok(Json::obj([
            ("$", Json::text("accessor")),
            ("get", Json::Bool(get.is_some())),
            ("set", Json::Bool(set.is_some())),
        ])),
        Prop::Data { value, .. } => encode(rt, rec, &value),
    }
}

/// The oracle's classOf for the kinds the kernel has. A host payload is rejected first (no encoding yet).
/// Arrays come next, then Error by prototype chain (`instanceof Error`), then plain objects.
fn class_of(rt: &Rt, o: ObjRef) -> Res<Class> {
    if rt.native_payload(o).is_some() {
        if promise_state(rt, &Value::Object(o)).is_some() {
            return Ok(Class::Promise);
        }
        if vn_value::builtins::map_set::map_entries(rt, o).is_some() {
            return Ok(Class::Map);
        }
        if vn_value::builtins::map_set::set_values(rt, o).is_some() {
            return Ok(Class::Set);
        }
        let proto = rt.get_prototype_of(o);
        if proto.is_some() && (proto == rt.builtin("MapIteratorPrototype") || proto == rt.builtin("SetIteratorPrototype")) {
            return Ok(Class::Object);
        }
        return Err(untraceable(rt));
    }
    if rt.is_array(&Value::Object(o)) {
        return Ok(Class::Array);
    }
    if inherits(rt, o, Intrinsic::ErrorPrototype) {
        return Ok(Class::Error);
    }
    Ok(Class::Object)
}

/// True when `target` is on the prototype chain of `o` (not counting `o` itself).
fn inherits(rt: &Rt, o: ObjRef, target: Intrinsic) -> bool {
    let wanted = rt.intrinsic(target);
    let mut cur = rt.get_prototype_of(o);
    while let Some(p) = cur {
        if p == wanted {
            return true;
        }
        cur = rt.get_prototype_of(p);
    }
    false
}

/// `"null"`, `"Object.prototype"`, or `"other"` (the oracle's protoOf).
fn proto_text(rt: &Rt, o: ObjRef) -> &'static str {
    match rt.get_prototype_of(o) {
        None => "null",
        Some(p) if p == rt.intrinsic(Intrinsic::ObjectPrototype) => "Object.prototype",
        Some(_) => "other",
    }
}

/// trace-format v1.1: `integrity` appears only on non-extensible objects, as frozen, else sealed, else
/// nonextensible. An empty non-extensible object is frozen (its integrity test holds vacuously).
fn integrity_of(rt: &Rt, o: ObjRef) -> Res<Option<&'static str>> {
    if rt.is_extensible(o)? {
        return Ok(None);
    }
    if rt.is_frozen(o)? {
        Ok(Some("frozen"))
    } else if rt.is_sealed(o)? {
        Ok(Some("sealed"))
    } else {
        Ok(Some("nonextensible"))
    }
}

fn array_length(rt: &Rt, o: ObjRef) -> Res<usize> {
    match rt.get_str(&Value::Object(o), "length")? {
        Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => Ok(n as usize),
        _ => Err(untraceable(rt)),
    }
}

/// `typeof v === 'string' ? v : String(v)` for an error's name or message. `String` describes symbols.
pub(crate) fn js_string_of(rt: &Rt, v: &Value) -> Res<JsStr> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Symbol(s) => {
            let mut units: Vec<u16> = "Symbol(".encode_utf16().collect();
            if let Some(d) = rt.symbol_description(*s) {
                units.extend_from_slice(d.units());
            }
            units.push(u16::from(b')'));
            Ok(JsStr::from_vec(units))
        }
        _ => rt.to_string(v),
    }
}

/// A property key as the JSON key text (array indices in decimal).
fn key_units(key: &PropKey) -> Vec<u16> {
    match key {
        PropKey::Index(i) => i.to_string().encode_utf16().collect(),
        PropKey::Str(s) => s.units().to_vec(),
        PropKey::Sym(_) => Vec::new(), // symbol keys are skipped before this is reached
    }
}

fn is_key(key: &PropKey, name: &str) -> bool {
    matches!(key, PropKey::Str(s) if s.units().iter().copied().eq(name.encode_utf16()))
}

#[cfg(test)]
mod tests {
    use super::number_json;

    #[test]
    fn number_forms() {
        assert_eq!(number_json(-0.0).to_json(), r#"{"$":"num","v":"-0"}"#);
        assert_eq!(number_json(f64::NAN).to_json(), r#"{"$":"num","v":"NaN"}"#);
        assert_eq!(number_json(f64::INFINITY).to_json(), r#"{"$":"num","v":"Infinity"}"#);
        assert_eq!(number_json(f64::NEG_INFINITY).to_json(), r#"{"$":"num","v":"-Infinity"}"#);
        assert_eq!(number_json(1e21).to_json(), "1e+21");
        assert_eq!(number_json(0.1).to_json(), "0.1");
    }
}
