// State snapshot and restore for `vue-native dev` (WP 3.7; the protocol is in main.rs). The state of the root component is
// the refs and the reactive objects among its <script setup> bindings: the setup result is read raw (toRaw of the setup
// state), so each binding is seen as the ref or reactive object it is. A snapshot keeps their plain data by binding name,
// every value tagged with its kind. Anything that is not plain data is left out and named in `dropped`: functions
// (closures), class instances, Map, Set, Promise and other native objects, host handles, symbols and cyclic references.
// Computed refs are derived from other state, and read-only refs cannot be assigned back: both are left out silently.
// A restore assigns a snapshot to a freshly mounted app: a binding
// that is missing, is no longer state, or has another kind is skipped and named in `skipped`; nothing is restored by guess.
//
// Tagged values: {"t":"number","v":<number>} (or "NaN", "Infinity", "-Infinity" as strings), {"t":"string","v":"..."},
// {"t":"boolean","v":true}, {"t":"null"}, {"t":"undefined"}, {"t":"array","v":[...]}, {"t":"object","v":{"key": ...}}.
// An element or property that is left out is omitted (array indices after it move down).

use vn_runtime::exports;
use vn_runtime::instance::instance_of;
use vn_testkit::json::Json;
use vn_value::{Intrinsic, JsStr, ObjClass, ObjRef, Prop, PropKey, Res, Rt, Value};

/// The state object (binding name -> tagged value) and the warnings of what a snapshot left out.
pub struct Snapshot {
    pub state: Json,
    pub dropped: Vec<String>,
}

/// The bindings that a restore assigned, and those it skipped with the reason.
#[derive(Default)]
pub struct Restore {
    pub restored: Vec<String>,
    pub skipped: Vec<(String, String)>,
}

// The exports of the vue module that classify a binding.
struct Exports {
    is_ref: Value,
    is_readonly: Value,
    is_reactive: Value,
    to_raw: Value,
}

impl Exports {
    fn new(rt: &Rt) -> Exports {
        let all = exports::all(rt);
        let get = |name: &str| {
            all.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("the vue module has no {name} (framework bug)"))
        };
        Exports { is_ref: get("isRef"), is_readonly: get("isReadonly"), is_reactive: get("isReactive"), to_raw: get("toRaw") }
    }

    fn test(&self, rt: &Rt, f: &Value, v: &Value) -> Res<bool> {
        Ok(rt.call(f, &Value::Undefined, std::slice::from_ref(v))?.to_boolean())
    }
}

// The raw bindings object of the root component: toRaw of its setup state. None when there is no component.
fn bindings_of(rt: &Rt, ex: &Exports, root: &Value) -> Res<Option<ObjRef>> {
    let Value::Object(_) = root else { return Ok(None) };
    let Value::Object(inst) = rt.get_str(root, "$")? else { return Ok(None) };
    let setup = instance_of(rt, inst).setup_state.borrow().clone();
    match rt.call(&ex.to_raw, &Value::Undefined, std::slice::from_ref(&setup))? {
        Value::Object(o) => Ok(Some(o)),
        _ => Ok(None),
    }
}

// A computed ref is recognized by its `effect`, which is the ref itself (@vue/reactivity 3.5); a plain ref has no effect.
// Restore must not assign a computed ref: the assignment would run its setter.
fn is_computed(rt: &Rt, binding: &Value) -> Res<bool> {
    let Value::Object(o) = binding else { return Ok(false) };
    Ok(matches!(rt.get_str(binding, "effect")?, Value::Object(e) if e == *o))
}

/// Snapshot of the root component's state. A component without a setup state gives an empty snapshot.
pub fn snapshot(rt: &Rt, root: &Value) -> Res<Snapshot> {
    let ex = Exports::new(rt);
    let mut snap = Snapshot { state: Json::Obj(Vec::new()), dropped: Vec::new() };
    let Some(bindings) = bindings_of(rt, &ex, root)? else { return Ok(snap) };
    let bindings_v = Value::Object(bindings);
    let mut pairs = Vec::new();
    for key in rt.own_keys(bindings)? {
        let Some(name) = key_name(&key) else { continue };
        if !is_enumerable(rt, bindings, &key)? {
            continue;
        }
        let binding = rt.get(bindings, &key, &bindings_v)?;
        let label = name.to_rust_string_lossy();
        let value = if ex.test(rt, &ex.is_ref, &binding)? {
            // A read-only ref cannot be restored by assignment, and a computed ref is derived: both are left out.
            if ex.test(rt, &ex.is_readonly, &binding)? || is_computed(rt, &binding)? {
                continue;
            }
            rt.get_str(&binding, "value")?
        } else if ex.test(rt, &ex.is_reactive, &binding)? {
            binding
        } else {
            continue;
        };
        let mut stack = Vec::new();
        if let Some(j) = encode(rt, &ex, &value, &label, &mut snap.dropped, &mut stack)? {
            pairs.push((name.units().to_vec(), j));
        }
    }
    snap.state = Json::Obj(pairs);
    Ok(snap)
}

/// Assigns a snapshot's bindings to the mounted root's bindings. Each restored value keeps the kind it was tagged with.
pub fn restore(rt: &Rt, root: &Value, state: &Json) -> Res<Restore> {
    let ex = Exports::new(rt);
    let mut out = Restore::default();
    let bindings = match bindings_of(rt, &ex, root)? {
        Some(b) => b,
        None => rt.new_object(),
    };
    let bindings_v = Value::Object(bindings);
    for (units, tagged) in pairs_of(state) {
        let name = String::from_utf16_lossy(units);
        let key = PropKey::from_str(&name);
        if !rt.has_own_property(bindings, &key)? {
            out.skipped.push((name, "no binding of this name in the new version".to_string()));
            continue;
        }
        let binding = rt.get(bindings, &key, &bindings_v)?;
        // A computed ref of the new version is derived: the saved value is not assigned to it (that would run its setter).
        if ex.test(rt, &ex.is_ref, &binding)? && is_computed(rt, &binding)? {
            out.skipped.push((name, "a computed ref, derived from other state (not restored)".to_string()));
            continue;
        }
        let is_ref = ex.test(rt, &ex.is_ref, &binding)? && !ex.test(rt, &ex.is_readonly, &binding)?;
        let current = if is_ref {
            rt.get_str(&binding, "value")?
        } else if ex.test(rt, &ex.is_reactive, &binding)? {
            binding.clone()
        } else {
            out.skipped.push((name, "no longer state in the new version (not a ref or a reactive object)".to_string()));
            continue;
        };
        let (want, have) = (tag_name(tagged), kind_name(rt, &current));
        if want != have {
            out.skipped.push((name, format!("type changed from {want} to {have}")));
            continue;
        }
        if is_ref {
            rt.set_v_strict(&binding, &PropKey::from_str("value"), decode(rt, tagged)?)?;
            out.restored.push(name);
            continue;
        }
        // A reactive object: property by property, each with the same kind check.
        let mut any = false;
        for (k_units, child) in pairs_of(tagged.field("v").unwrap_or(&Json::Null)) {
            let prop = format!("{name}.{}", String::from_utf16_lossy(k_units));
            let k = PropKey::from_str(&String::from_utf16_lossy(k_units));
            let Some(obj) = current.as_object() else { continue };
            if !rt.has_own_property(obj, &k)? {
                out.skipped.push((prop, "no property of this name in the new version".to_string()));
                continue;
            }
            let now = rt.get(obj, &k, &current)?;
            let (want, have) = (tag_name(child), kind_name(rt, &now));
            if want != have {
                out.skipped.push((prop, format!("type changed from {want} to {have}")));
                continue;
            }
            rt.set_v_strict(&current, &k, decode(rt, child)?)?;
            any = true;
        }
        if any {
            out.restored.push(name);
        }
    }
    Ok(out)
}

// The tagged JSON of a value, or None when it is left out (its path is then named in `dropped`).
fn encode(rt: &Rt, ex: &Exports, v: &Value, path: &str, dropped: &mut Vec<String>, stack: &mut Vec<ObjRef>) -> Res<Option<Json>> {
    Ok(Some(match v {
        Value::Undefined => bare("undefined"),
        Value::Null => bare("null"),
        Value::Bool(b) => tag("boolean", Json::Bool(*b)),
        Value::Number(n) if n.is_finite() => tag("number", Json::Num(rt.to_string(v)?.to_rust_string_lossy())),
        Value::Number(n) => tag("number", Json::text(if n.is_nan() { "NaN" } else if *n > 0.0 { "Infinity" } else { "-Infinity" })),
        Value::String(s) => tag("string", Json::Str(s.units().to_vec())),
        Value::Object(o) => return encode_object(rt, ex, *o, path, dropped, stack),
        Value::Symbol(_) => {
            dropped.push(format!("{path}: symbol (not serialized)"));
            return Ok(None);
        }
        Value::Uninit | Value::Hole => {
            dropped.push(format!("{path}: value not yet initialized (not serialized)"));
            return Ok(None);
        }
    }))
}

fn encode_object(rt: &Rt, ex: &Exports, o: ObjRef, path: &str, dropped: &mut Vec<String>, stack: &mut Vec<ObjRef>) -> Res<Option<Json>> {
    let v = Value::Object(o);
    if rt.is_callable(&v) {
        dropped.push(format!("{path}: function (closure, not serialized)"));
        return Ok(None);
    }
    if stack.contains(&o) {
        dropped.push(format!("{path}: circular reference (not serialized)"));
        return Ok(None);
    }
    stack.push(o);
    let out = if rt.is_array(&v) {
        let len = rt.to_number(&rt.get_str(&v, "length")?)? as u32;
        let mut items = Vec::new();
        for i in 0..len {
            let item = rt.get_v(&v, &PropKey::Index(i))?;
            if let Some(j) = encode(rt, ex, &item, &format!("{path}[{i}]"), dropped, stack)? {
                items.push(j);
            }
        }
        Some(tag("array", Json::Arr(items)))
    } else if is_plain(rt, ex, o)? {
        let mut pairs = Vec::new();
        for key in rt.own_keys(o)? {
            let Some(name) = key_name(&key) else { continue };
            if !is_enumerable(rt, o, &key)? {
                continue;
            }
            let child = rt.get(o, &key, &v)?;
            if let Some(j) = encode(rt, ex, &child, &format!("{path}.{}", name.to_rust_string_lossy()), dropped, stack)? {
                pairs.push((name.units().to_vec(), j));
            }
        }
        Some(tag("object", Json::Obj(pairs)))
    } else {
        dropped.push(format!("{path}: {} (not serialized)", kind_of_object(rt, o)));
        None
    };
    stack.pop();
    Ok(out)
}

// A plain object: an ordinary object, or a reactive proxy of one, whose prototype is Object.prototype (or none).
fn is_plain(rt: &Rt, ex: &Exports, o: ObjRef) -> Res<bool> {
    let class_ok = match rt.class_of(o) {
        ObjClass::Ordinary => true,
        ObjClass::Exotic => ex.test(rt, &ex.is_reactive, &Value::Object(o))?,
        _ => false,
    };
    let proto_ok = match rt.get_prototype_of(o) {
        None => true,
        Some(p) => p == rt.intrinsic(Intrinsic::ObjectPrototype),
    };
    Ok(class_ok && proto_ok)
}

fn kind_of_object(rt: &Rt, o: ObjRef) -> &'static str {
    match rt.class_of(o) {
        ObjClass::Native => "a native object (Map, Set, Promise or a host handle)",
        ObjClass::Error => "an error object",
        ObjClass::Exotic => "an exotic object",
        ObjClass::Wrapper => "a wrapper object",
        _ => "a class instance",
    }
}

// The name of a property key as a string (array indices included); symbols have none.
fn key_name(key: &PropKey) -> Option<JsStr> {
    match key {
        PropKey::Str(s) => Some(s.clone()),
        PropKey::Index(i) => Some(JsStr::from_str(&i.to_string())),
        PropKey::Sym(_) => None,
    }
}

fn is_enumerable(rt: &Rt, o: ObjRef, key: &PropKey) -> Res<bool> {
    Ok(match rt.get_own_property(o, key)? {
        Some(Prop::Data { flags, .. }) | Some(Prop::Accessor { flags, .. }) => flags.enumerable,
        None => false,
    })
}

// The kind a tagged value is restored as: the same names as the tags, and "function" for a callable value.
fn kind_name(rt: &Rt, v: &Value) -> &'static str {
    match v {
        Value::Undefined => "undefined",
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Object(_) if rt.is_callable(v) => "function",
        Value::Object(_) if rt.is_array(v) => "array",
        Value::Object(_) => "object",
        _ => "other",
    }
}

fn decode(rt: &Rt, j: &Json) -> Res<Value> {
    Ok(match tag_name(j).as_str() {
        "undefined" => Value::Undefined,
        "null" => Value::Null,
        "boolean" => Value::Bool(matches!(j.field("v"), Some(Json::Bool(true)))),
        "number" => Value::Number(match j.field("v") {
            Some(Json::Num(text)) => text.parse::<f64>().unwrap_or(f64::NAN),
            Some(Json::Str(units)) => match String::from_utf16_lossy(units).as_str() {
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => f64::NAN,
            },
            _ => f64::NAN,
        }),
        "string" => Value::String(JsStr::from_units(j.field("v").and_then(Json::units).unwrap_or(&[]))),
        "array" => {
            let mut elems = Vec::new();
            for item in j.field("v").and_then(Json::items).unwrap_or(&[]) {
                elems.push(decode(rt, item)?);
            }
            Value::Object(rt.new_array(elems))
        }
        _ => {
            let o = rt.new_object();
            for (k, child) in pairs_of(j.field("v").unwrap_or(&Json::Null)) {
                rt.create_data_property(o, PropKey::from_str(&String::from_utf16_lossy(k)), decode(rt, child)?)?;
            }
            Value::Object(o)
        }
    })
}

fn tag(t: &str, v: Json) -> Json {
    Json::obj([("t", Json::text(t)), ("v", v)])
}

fn bare(t: &str) -> Json {
    Json::obj([("t", Json::text(t))])
}

fn tag_name(j: &Json) -> String {
    j.field("t").and_then(Json::units).map(String::from_utf16_lossy).unwrap_or_default()
}

// The (key units, value) pairs of a JSON object; none for anything else.
fn pairs_of(j: &Json) -> Vec<(&[u16], &Json)> {
    match j {
        Json::Obj(pairs) => pairs.iter().map(|(k, v)| (k.as_slice(), v)).collect(),
        _ => Vec::new(),
    }
}
