//! Helpers shared by the reactivity modules. The predicates follow @vue/reactivity 3.5.43 (`isRef`, `isShallow`,
//! `isProxy`, `toRaw`, ...) and @vue/shared 3.5.43 (`isIntegerKey`, `isPlainObject`, `hasChanged`, `remove`). They read
//! properties, so they can run user getters and return `Res`.

use std::collections::HashSet;

use vn_value::{CodeFn, Flags, JsStr, ObjRef, Prop, PropKey, Res, Rt, Value};

use crate::state::{state, to_reactive};

/// `hasChanged` (@vue/shared 3.5.43): `!Object.is(value, oldValue)`.
pub(crate) fn has_changed(a: &Value, b: &Value) -> bool {
    !vn_value::same_value(a, b)
}

pub(crate) fn truthy(v: &Value) -> bool {
    v.to_boolean()
}

/// `isObject`: a non-callable object.
pub(crate) fn is_object(rt: &Rt, v: &Value) -> bool {
    matches!(v, Value::Object(_)) && !rt.is_callable(v)
}

/// A native function object whose closure data is read with `fn_data(c.callee)`.
pub(crate) fn native(rt: &Rt, name: &str, length: u32, code: CodeFn, data: Vec<Value>) -> ObjRef {
    rt.new_native_function(code, name, length, data)
}

/// Defines an accessor on a prototype (non-enumerable, configurable), as a class getter/setter pair.
pub(crate) fn accessor(rt: &Rt, obj: ObjRef, name: &str, get: CodeFn, set: Option<CodeFn>) {
    let g = rt.new_native_function(get, &format!("get {name}"), 0, Vec::new());
    let s = set.map(|f| rt.new_native_function(f, &format!("set {name}"), 1, Vec::new()));
    rt.define_accessor(obj, PropKey::from_str(name), Some(g), s, Flags::new(false, false, true))
        .expect("reactivity prototype accessor (framework bug)");
}

/// Defines a non-enumerable, writable, configurable data property on a prototype.
pub(crate) fn hidden(rt: &Rt, obj: ObjRef, name: &str, value: Value) {
    rt.define_data(obj, PropKey::from_str(name), value, Flags::HIDDEN)
        .expect("reactivity prototype data (framework bug)");
}

/// `isRef` (3.5.43): `r ? r["__v_isRef"] === true : false`.
pub(crate) fn is_ref(rt: &Rt, v: &Value) -> Res<bool> {
    if !truthy(v) {
        return Ok(false);
    }
    Ok(matches!(rt.get_str(v, "__v_isRef")?, Value::Bool(true)))
}

/// `isShallow`: `!!(value && value["__v_isShallow"])`.
pub(crate) fn is_shallow(rt: &Rt, v: &Value) -> Res<bool> {
    if !truthy(v) {
        return Ok(false);
    }
    Ok(rt.get_str(v, "__v_isShallow")?.to_boolean())
}

/// `isReadonly`: `!!(value && value["__v_isReadonly"])`.
pub(crate) fn is_readonly(rt: &Rt, v: &Value) -> Res<bool> {
    if !truthy(v) {
        return Ok(false);
    }
    Ok(rt.get_str(v, "__v_isReadonly")?.to_boolean())
}

/// `isReactive`: readonly proxies answer for their raw target; otherwise `__v_isReactive`.
pub(crate) fn is_reactive(rt: &Rt, v: &Value) -> Res<bool> {
    if is_readonly(rt, v)? {
        let raw = rt.get_str(v, "__v_raw")?;
        return is_reactive(rt, &raw);
    }
    if !truthy(v) {
        return Ok(false);
    }
    Ok(rt.get_str(v, "__v_isReactive")?.to_boolean())
}

/// `isProxy`: `value ? !!value["__v_raw"] : false`.
pub(crate) fn is_proxy(rt: &Rt, v: &Value) -> Res<bool> {
    if !truthy(v) {
        return Ok(false);
    }
    Ok(rt.get_str(v, "__v_raw")?.to_boolean())
}

/// `toRaw`: follows `__v_raw` until an object without it.
pub(crate) fn to_raw(rt: &Rt, v: &Value) -> Res<Value> {
    if !truthy(v) {
        return Ok(v.clone());
    }
    let raw = rt.get_str(v, "__v_raw")?;
    if truthy(&raw) {
        to_raw(rt, &raw)
    } else {
        Ok(v.clone())
    }
}

/// `toReactive` through the replaceable hook (identity until a reactive wrapper is registered).
pub(crate) fn wrap_reactive(rt: &Rt, v: &Value) -> Res<Value> {
    to_reactive(rt, v)
}

/// `isIntegerKey` (@vue/shared 3.5.43): a string whose `"" + parseInt(key, 10)` equals the key.
pub(crate) fn is_integer_key(rt: &Rt, v: &Value) -> bool {
    let Value::String(s) = v else {
        return false;
    };
    let units = s.units();
    if units.is_empty() || units.iter().any(|&c| !(0x30..=0x39).contains(&c)) {
        return false;
    }
    let text: String = units.iter().map(|&c| c as u8 as char).collect();
    let n: f64 = text.parse().unwrap_or(f64::NAN);
    match rt.to_string(&Value::Number(n)) {
        Ok(t) => t == *s,
        Err(_) => false,
    }
}

/// `Object.prototype.toString.call(v)` with the builtin captured when the reactivity state was created.
pub(crate) fn to_type_string(rt: &Rt, v: &Value) -> Res<JsStr> {
    let f = state(rt).object_to_string();
    match rt.call(&Value::Object(f), v, &[])? {
        Value::String(s) => Ok(s),
        _ => Ok(JsStr::empty()),
    }
}

/// `isMap` / `isSet` / `isPlainObject` of @vue/shared: the `toString` tag of the value.
pub(crate) fn has_tag(rt: &Rt, v: &Value, tag: &str) -> Res<bool> {
    Ok(to_type_string(rt, v)? == JsStr::from_str(&format!("[object {tag}]")))
}

/// Property key as a JS value (`String(key)` for indices, the symbol itself for symbols).
pub(crate) fn key_value(k: &PropKey) -> Value {
    match k {
        PropKey::Index(i) => Value::String(JsStr::from_str(&i.to_string())),
        PropKey::Str(s) => Value::String(s.clone()),
        PropKey::Sym(s) => Value::Symbol(*s),
    }
}

/// Keys of `for (const k in o)`: enumerable string keys along the prototype chain, shadowed names skipped.
pub(crate) fn for_in_keys(rt: &Rt, o: ObjRef) -> Res<Vec<PropKey>> {
    let mut seen: HashSet<PropKey> = HashSet::new();
    let mut out = Vec::new();
    let mut cur = Some(o);
    while let Some(obj) = cur {
        for k in rt.own_keys(obj)? {
            if matches!(k, PropKey::Sym(_)) || !seen.insert(k.clone()) {
                continue;
            }
            if let Some(p) = rt.get_own_property(obj, &k)? {
                if p.flags().enumerable {
                    out.push(k);
                }
            }
        }
        cur = rt.get_prototype_of(obj);
    }
    Ok(out)
}
